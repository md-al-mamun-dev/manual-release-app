use actix_web::web;

use crate::error::ApiError;

pub mod environments;
pub mod health;
pub mod project_inspections;
pub mod projects;

pub fn configure(config: &mut web::ServiceConfig) {
    let json_config = web::JsonConfig::default().error_handler(|error, _request| {
        ApiError::Validation(format!("invalid JSON request: {error}")).into()
    });

    let path_config = web::PathConfig::default().error_handler(|error, _request| {
        ApiError::Validation(format!("invalid path parameter: {error}")).into()
    });

    config.service(
        web::scope("/api")
            .app_data(json_config)
            .app_data(path_config)
            .service(health::health)
            .configure(projects::configure)
            .configure(project_inspections::configure)
            .configure(environments::configure),
    );
}
