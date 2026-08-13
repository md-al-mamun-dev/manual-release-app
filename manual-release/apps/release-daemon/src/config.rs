use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub backend_host: String,
    pub backend_port: u16,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;

        let backend_host =
            env::var("BACKEND_HOST").map_err(|_| anyhow::anyhow!("BACKEND_HOST is required"))?;

        let backend_port = env::var("BACKEND_PORT")
            .map_err(|_| anyhow::anyhow!("BACKEND_PORT is required"))?
            .parse::<u16>()
            .map_err(|_| anyhow::anyhow!("BACKEND_PORT must be a valid TCP port"))?;

        Ok(Self {
            database_url,
            backend_host,
            backend_port,
        })
    }
}
