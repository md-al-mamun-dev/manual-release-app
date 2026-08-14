use actix_web::{App, HttpServer, middleware, web};

use release_daemon::{
    app_state::AppState,
    config::AppConfig,
    db,
    executor::command_executor::CommandExecutor,
    inspection::project_inspector::ProjectInspector,
    repositories::{
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository,
    },
    routes,
    services::{
        project_inspection_service::ProjectInspectionService, project_service::ProjectService,
    },
};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("release_daemon=info".parse()?),
        )
        .init();

    let config = AppConfig::from_env()?;

    let pool = db::create_pool(&config.database_url).await?;

    tracing::info!("database connection established");

    let project_repository = ProjectRepository::new(pool.clone());
    let project_service = ProjectService::new(project_repository.clone());

    let command_executor = CommandExecutor::new(1024 * 1024);
    let project_inspector = ProjectInspector::new(command_executor);

    let inspection_repository = ProjectInspectionRepository::new(pool.clone());
    let project_inspection_service =
        ProjectInspectionService::new(project_repository, inspection_repository, project_inspector);

    let state = web::Data::new(AppState {
        pool,
        project_service,
        project_inspection_service,
    });

    let bind_address = format!("{}:{}", config.backend_host, config.backend_port);

    tracing::info!(
        address = %bind_address,
        "starting release daemon"
    );

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(routes::configure)
    })
    .bind(&bind_address)?
    .run()
    .await?;

    Ok(())
}
