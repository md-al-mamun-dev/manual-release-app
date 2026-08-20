use actix_web::{App, HttpServer, middleware, web};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use release_daemon::{
    app_state::AppState,
    config::AppConfig,
    db,
    executor::command_executor::CommandExecutor,
    inspection::project_inspector::ProjectInspector,
    openapi::ApiDoc,
    repositories::{
        environment_repository::EnvironmentRepository,
        project_inspection_repository::ProjectInspectionRepository,
        project_repository::ProjectRepository, release_repository::ReleaseRepository,
    },
    routes,
    services::{
        environment_service::EnvironmentService,
        project_inspection_service::ProjectInspectionService, project_service::ProjectService,
        release_service::ReleaseService,
    },
    worker::JobWorker,
};
use tokio_util::sync::CancellationToken;

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
    let inspection_repository_clone = inspection_repository.clone();

    let project_inspection_service = ProjectInspectionService::new(
        project_repository.clone(),
        inspection_repository,
        project_inspector,
    );

    let environment_repository = EnvironmentRepository::new(pool.clone());
    let environment_service =
        EnvironmentService::new(environment_repository, project_repository.clone());

    let release_repository = ReleaseRepository::new(pool.clone());
    let release_service = ReleaseService::new(
        project_repository.clone(),
        inspection_repository_clone,
        release_repository,
    );

    let state = web::Data::new(AppState {
        pool,
        project_service,
        project_inspection_service,
        environment_service,
        release_service,
    });

    let bind_address = format!("{}:{}", config.backend_host, config.backend_port);

    tracing::info!(
        address = %bind_address,
        "starting release daemon"
    );

    let cancel_token = CancellationToken::new();
    let worker = JobWorker::new(cancel_token.clone());
    let worker_handle = tokio::spawn(async move {
        worker.run().await;
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::Logger::default())
            .configure(routes::configure)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}")
                    .url("/api-docs/openapi.json", ApiDoc::openapi()),
            )
    })
    .disable_signals()
    .bind(&bind_address)?
    .run();

    let server_handle = server.handle();
    let server_task = tokio::spawn(server);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received SIGINT (ctrl-c), shutting down");
        }
    }

    tracing::info!("initiating graceful shutdown...");
    cancel_token.cancel();
    server_handle.stop(true).await;

    let _ = tokio::join!(worker_handle, server_task);
    tracing::info!("shutdown complete");

    Ok(())
}
