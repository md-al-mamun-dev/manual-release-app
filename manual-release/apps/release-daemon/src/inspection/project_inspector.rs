use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::{
    domain::project_inspection::{GitInspection, NodeInspection, ProjectInspectionReport},
    executor::command_executor::{CommandExecutor, CommandOutput},
};

#[derive(Clone)]
pub struct ProjectInspector {
    executor: CommandExecutor,
}

impl ProjectInspector {
    pub fn new(executor: CommandExecutor) -> Self {
        Self { executor }
    }

    pub async fn inspect(&self, repository_path: &str) -> anyhow::Result<ProjectInspectionReport> {
        let root = fs::canonicalize(repository_path).with_context(|| {
            format!(
                "repository path does not exist: \
                 {repository_path}"
            )
        })?;

        if !root.is_dir() {
            bail!("repository path is not a directory");
        }

        let git = self.inspect_git(&root).await?;

        let mut warnings = Vec::new();

        let lockfiles = detect_lockfiles(&root);

        let package_manager = detect_package_manager(&lockfiles, &mut warnings);

        let node = inspect_node(&root, &mut warnings)?;

        let runtimes = detect_runtimes(&root);

        let dockerfiles = detect_dockerfiles(&root);

        let compose_files = detect_compose_files(&root);

        let github_workflows = detect_github_workflows(&root)?;

        Ok(ProjectInspectionReport {
            repository_path: root.to_string_lossy().into_owned(),

            git,

            runtimes,

            package_manager,

            lockfiles,

            node,

            dockerfiles,

            compose_files,

            github_workflows,

            warnings,
        })
    }

    async fn inspect_git(&self, root: &Path) -> anyhow::Result<GitInspection> {
        let inside = self
            .git(root, &["rev-parse", "--is-inside-work-tree"])
            .await?;

        if inside.stdout.trim() != "true" {
            bail!("path is not inside a Git worktree");
        }

        let commit = self
            .git(root, &["rev-parse", "HEAD"])
            .await?
            .stdout
            .trim()
            .to_string();

        if !is_valid_git_commit(&commit) {
            bail!("Git returned an invalid commit SHA");
        }

        let branch_output = self.git(root, &["branch", "--show-current"]).await?;

        let branch = match branch_output.stdout.trim() {
            "" => None,

            value => Some(value.to_string()),
        };

        let status = self
            .git(
                root,
                &["status", "--porcelain=v1", "--untracked-files=normal"],
            )
            .await?;

        Ok(GitInspection {
            commit,

            branch,

            dirty: !status.stdout.trim().is_empty(),
        })
    }

    async fn git(&self, cwd: &Path, args: &[&str]) -> anyhow::Result<CommandOutput> {
        let output = self
            .executor
            .run("git", args, cwd, Duration::from_secs(10))
            .await?;

        if output.exit_code != Some(0) {
            bail!("Git command failed: {}", output.stderr.trim());
        }

        Ok(output)
    }
}

fn detect_lockfiles(root: &Path) -> Vec<String> {
    [
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
    ]
    .iter()
    .filter(|name| is_regular_file(&root.join(name)))
    .map(|name| (*name).to_string())
    .collect()
}

fn detect_package_manager(lockfiles: &[String], warnings: &mut Vec<String>) -> Option<String> {
    if lockfiles.len() > 1 {
        warnings.push(format!(
            "multiple JavaScript lockfiles detected: {}",
            lockfiles.join(", ")
        ));
        return None;
    }

    match lockfiles.first().map(String::as_str) {
        Some("package-lock.json") => Some("npm".to_string()),
        Some("pnpm-lock.yaml") => Some("pnpm".to_string()),
        Some("yarn.lock") => Some("yarn".to_string()),
        Some("bun.lock") | Some("bun.lockb") => Some("bun".to_string()),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    #[serde(rename = "packageManager")]
    package_manager: Option<String>,
    engines: Option<PackageEngines>,
    scripts: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct PackageEngines {
    node: Option<String>,
}

fn inspect_node(root: &Path, warnings: &mut Vec<String>) -> anyhow::Result<Option<NodeInspection>> {
    let path = root.join("package.json");

    if !is_regular_file(&path) {
        return Ok(None);
    }

    let metadata = fs::metadata(&path)?;
    const MAX_PACKAGE_JSON_BYTES: u64 = 2 * 1024 * 1024;

    if metadata.len() > MAX_PACKAGE_JSON_BYTES {
        bail!("package.json exceeds the maximum supported size");
    }

    let contents = fs::read_to_string(&path)?;
    let package: PackageJson =
        serde_json::from_str(&contents).context("package.json contains invalid JSON")?;

    if package.package_manager.is_none() {
        warnings.push("package.json does not define packageManager".to_string());
    }

    Ok(Some(NodeInspection {
        package_manager_field: package.package_manager,
        engines_node: package.engines.and_then(|engines| engines.node),
        scripts: package.scripts.unwrap_or_default(),
    }))
}

fn detect_runtimes(root: &Path) -> Vec<String> {
    let mut runtimes = Vec::new();

    if is_regular_file(&root.join("package.json")) {
        runtimes.push("NODE".to_string());
    }

    if is_regular_file(&root.join("pyproject.toml"))
        || is_regular_file(&root.join("requirements.txt"))
        || is_regular_file(&root.join("poetry.lock"))
        || is_regular_file(&root.join("uv.lock"))
    {
        runtimes.push("PYTHON".to_string());
    }

    runtimes
}

fn detect_dockerfiles(root: &Path) -> Vec<String> {
    let mut result = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return result;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        if (name == "Dockerfile" || name.starts_with("Dockerfile."))
            && is_regular_file(&entry.path())
        {
            result.push(name.into_owned());
        }
    }
    result.sort();
    result
}

fn detect_compose_files(root: &Path) -> Vec<String> {
    [
        "compose.yml",
        "compose.yaml",
        "docker-compose.yml",
        "docker-compose.yaml",
    ]
    .iter()
    .filter(|name| is_regular_file(&root.join(name)))
    .map(|name| (*name).to_string())
    .collect()
}

fn detect_github_workflows(root: &Path) -> anyhow::Result<Vec<String>> {
    let directory = root.join(".github/workflows");

    if !directory.is_dir() {
        return Ok(Vec::new());
    }

    let mut workflows = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !is_regular_file(&entry.path()) {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".yml") || name.ends_with(".yaml") {
            workflows.push(name);
        }
    }
    workflows.sort();

    Ok(workflows)
}

fn is_regular_file(path: &PathBuf) -> bool {
    match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type().is_file(),
        Err(_) => false,
    }
}

fn is_valid_git_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
