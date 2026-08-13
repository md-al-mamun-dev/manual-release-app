use actix_web::web;

pub mod health;
pub mod projects;

pub fn configure(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/api")
            .service(health::health)
            .configure(projects::configure),
    );
}
