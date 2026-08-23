use serde::{Deserialize, Serialize};

use crate::domain::project_build_config::{ApplicationType, Framework, PackageManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandData {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandData {
    pub fn new(program: String, args: Vec<String>) -> Result<Self, &'static str> {
        let prohibited_programs = [
            "bash", "sh", "eval", "python", "node", "ruby", "perl", "php",
        ];

        if prohibited_programs.contains(&program.as_str()) {
            return Err("Prohibited executable program in build plan");
        }

        if args.iter().any(|arg| arg == "-c" || arg == "--eval") {
            return Err("Prohibited arguments (-c, --eval) in build plan");
        }

        Ok(Self { program, args })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPlan {
    pub source_sha: String,
    pub application_type: ApplicationType,
    pub framework: Framework,
    pub runtime: String,
    pub package_manager: PackageManager,
    pub install_command: Option<CommandData>,
    pub lint_command: Option<CommandData>,
    pub typecheck_command: Option<CommandData>,
    pub test_command: Option<CommandData>,
    pub build_command: Option<CommandData>,
    pub build_image_command: Option<CommandData>,
    pub test_image_command: Option<CommandData>,
    pub dockerfile: Option<String>,
    pub docker_context: Option<String>,
    pub application_port: Option<i32>,
    pub health_endpoint: Option<String>,
}
