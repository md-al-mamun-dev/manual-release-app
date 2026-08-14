use actix_web::web;

pub mod health;
pub mod project_inspections;
pub mod projects;

pub fn configure(config: &mut web::ServiceConfig) {
    let json_config = web::JsonConfig::default()
        .error_handler(|err, _req| crate::error::ApiError::Validation(err.to_string()).into());

    config.app_data(json_config).service(
        web::scope("/api")
            .service(health::health)
            .configure(project_inspections::configure)
            .configure(projects::configure),
    );
}
