use crate::domain::project_build_config::{ApplicationType, Framework, PackageManager};
use crate::error::ApiError;
use std::path::Path;

pub struct BuildConfigValidator;

impl BuildConfigValidator {
    pub fn validate_compatibility(
        application_type: ApplicationType,
        framework: Framework,
        package_manager: PackageManager,
    ) -> Result<(), ApiError> {
        match application_type {
            ApplicationType::Node => {
                if !matches!(
                    package_manager,
                    PackageManager::Npm | PackageManager::Pnpm | PackageManager::Yarn
                ) {
                    return Err(ApiError::Validation(
                        "Node applications must use NPM, PNPM, or YARN".to_string(),
                    ));
                }

                if matches!(framework, Framework::Fastapi) {
                    return Err(ApiError::Validation(
                        "Node applications cannot use Python frameworks like FastAPI".to_string(),
                    ));
                }
            }
            ApplicationType::Python => {
                if !matches!(package_manager, PackageManager::Uv | PackageManager::Pip) {
                    return Err(ApiError::Validation(
                        "Python applications must use UV or PIP".to_string(),
                    ));
                }

                if matches!(
                    framework,
                    Framework::Nextjs | Framework::Nestjs | Framework::Express
                ) {
                    return Err(ApiError::Validation(
                        "Python applications cannot use Node frameworks".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn validate_path(path: Option<&str>, field_name: &str) -> Result<(), ApiError> {
        let Some(path_str) = path else {
            return Ok(());
        };

        if path_str.is_empty() {
            return Err(ApiError::Validation(format!(
                "{} cannot be empty",
                field_name
            )));
        }

        let p = Path::new(path_str);
        if p.is_absolute() || path_str.starts_with('/') {
            return Err(ApiError::Validation(format!(
                "{} must be a relative path",
                field_name
            )));
        }

        for component in p.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(ApiError::Validation(format!(
                    "{} cannot contain directory traversal (../)",
                    field_name
                )));
            }
        }

        Ok(())
    }

    pub fn validate_health_endpoint(endpoint: Option<&str>) -> Result<(), ApiError> {
        let Some(ep) = endpoint else {
            return Ok(());
        };

        if ep.is_empty() {
            return Err(ApiError::Validation(
                "health_endpoint cannot be empty".to_string(),
            ));
        }

        if !ep.starts_with('/') {
            return Err(ApiError::Validation(
                "health_endpoint must start with '/'".to_string(),
            ));
        }

        Ok(())
    }

    pub fn validate_port(port: Option<i32>) -> Result<(), ApiError> {
        if port.is_some_and(|p| !(1..=65535).contains(&p)) {
            return Err(ApiError::Validation(
                "application_port must be between 1 and 65535".to_string(),
            ));
        }
        Ok(())
    }
}
