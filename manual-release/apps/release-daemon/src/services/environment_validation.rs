use crate::{
    domain::environment::{CreateEnvironmentRequest, UpdateEnvironmentRequest},
    error::ApiError,
};

pub fn validate_create(input: &mut CreateEnvironmentRequest) -> Result<(), ApiError> {
    normalize_create(input);

    validate_common(
        &input.name,
        &input.ssh_host,
        input.ssh_port,
        &input.ssh_user,
        &input.remote_app_directory,
        input.server_architecture.as_deref(),
        input.ssh_identity_secret_ref.as_deref(),
        input.registry_credential_secret_ref.as_deref(),
        input.remote_env_file_path.as_deref(),
    )
}

pub fn validate_update(input: &mut UpdateEnvironmentRequest) -> Result<(), ApiError> {
    normalize_update(input);

    validate_common(
        &input.name,
        &input.ssh_host,
        input.ssh_port,
        &input.ssh_user,
        &input.remote_app_directory,
        input.server_architecture.as_deref(),
        input.ssh_identity_secret_ref.as_deref(),
        input.registry_credential_secret_ref.as_deref(),
        input.remote_env_file_path.as_deref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_common(
    name: &str,
    ssh_host: &str,
    ssh_port: u16,
    ssh_user: &str,
    remote_app_directory: &str,
    server_architecture: Option<&str>,
    ssh_identity_secret_ref: Option<&str>,
    registry_credential_secret_ref: Option<&str>,
    remote_env_file_path: Option<&str>,
) -> Result<(), ApiError> {
    validate_name(name)?;

    validate_ssh_host(ssh_host)?;

    validate_ssh_port(ssh_port)?;

    validate_ssh_user(ssh_user)?;

    validate_remote_path(remote_app_directory, "remoteAppDirectory")?;

    validate_optional_non_empty(server_architecture, "serverArchitecture")?;

    validate_optional_non_empty(ssh_identity_secret_ref, "sshIdentitySecretRef")?;

    validate_optional_non_empty(
        registry_credential_secret_ref,
        "registryCredentialSecretRef",
    )?;

    if let Some(path) = remote_env_file_path {
        validate_remote_path(path, "remoteEnvFilePath")?;
    }

    Ok(())
}

fn normalize_create(input: &mut CreateEnvironmentRequest) {
    input.name = input.name.trim().to_string();

    input.ssh_host = input.ssh_host.trim().to_string();

    input.ssh_user = input.ssh_user.trim().to_string();

    input.remote_app_directory = input.remote_app_directory.trim().to_string();

    input.server_architecture = normalize_optional(input.server_architecture.take());

    input.ssh_identity_secret_ref = normalize_optional(input.ssh_identity_secret_ref.take());

    input.registry_credential_secret_ref =
        normalize_optional(input.registry_credential_secret_ref.take());

    input.remote_env_file_path = normalize_optional(input.remote_env_file_path.take());
}

fn normalize_update(input: &mut UpdateEnvironmentRequest) {
    input.name = input.name.trim().to_string();

    input.ssh_host = input.ssh_host.trim().to_string();

    input.ssh_user = input.ssh_user.trim().to_string();

    input.remote_app_directory = input.remote_app_directory.trim().to_string();

    input.server_architecture = normalize_optional(input.server_architecture.take());

    input.ssh_identity_secret_ref = normalize_optional(input.ssh_identity_secret_ref.take());

    input.registry_credential_secret_ref =
        normalize_optional(input.registry_credential_secret_ref.take());

    input.remote_env_file_path = normalize_optional(input.remote_env_file_path.take());
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() {
        return Err(ApiError::Validation("name is required".to_string()));
    }

    if name.chars().count() > 100 {
        return Err(ApiError::Validation(
            "name must not exceed 100 characters".to_string(),
        ));
    }

    Ok(())
}

fn validate_ssh_port(port: u16) -> Result<(), ApiError> {
    if port == 0 {
        return Err(ApiError::Validation(
            "sshPort must be between 1 and 65535".to_string(),
        ));
    }

    Ok(())
}

fn validate_ssh_user(user: &str) -> Result<(), ApiError> {
    if user.is_empty() {
        return Err(ApiError::Validation("sshUser is required".to_string()));
    }

    if user.len() > 64 {
        return Err(ApiError::Validation("sshUser is too long".to_string()));
    }

    let valid = user
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.'));

    if !valid {
        return Err(ApiError::Validation(
            "sshUser contains unsupported characters".to_string(),
        ));
    }

    Ok(())
}

fn validate_ssh_host(host: &str) -> Result<(), ApiError> {
    if host.is_empty() {
        return Err(ApiError::Validation("sshHost is required".to_string()));
    }
    if host.chars().count() > 255 {
        return Err(ApiError::Validation(
            "sshHost must not exceed 255 characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_remote_path(path: &str, field_name: &str) -> Result<(), ApiError> {
    if path.is_empty() {
        return Err(ApiError::Validation(format!("{} is required", field_name)));
    }
    if path.chars().count() > 500 {
        return Err(ApiError::Validation(format!(
            "{} must not exceed 500 characters",
            field_name
        )));
    }
    if !path.starts_with('/') {
        return Err(ApiError::Validation(format!(
            "{} must be an absolute path",
            field_name
        )));
    }
    Ok(())
}

fn validate_optional_non_empty(value: Option<&str>, field_name: &str) -> Result<(), ApiError> {
    if let Some(val) = value {
        if val.is_empty() {
            return Err(ApiError::Validation(format!(
                "{} must not be empty if provided",
                field_name
            )));
        }
        if val.chars().count() > 255 {
            return Err(ApiError::Validation(format!(
                "{} must not exceed 255 characters",
                field_name
            )));
        }
    }
    Ok(())
}
