/// Server configuration, loaded from environment variables.
pub struct Config {
    pub database_url: String,
    pub sidecar_url: String,
    pub jwt_secret: String,
    pub s3_bucket: String,
    pub s3_endpoint: Option<String>,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost/cadmus".to_string()),
            sidecar_url: std::env::var("SIDECAR_URL")
                .unwrap_or_else(|_| "http://localhost:3001".to_string()),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".to_string()),
            s3_bucket: std::env::var("S3_BUCKET")
                .unwrap_or_else(|_| "cadmus-documents".to_string()),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
        }
    }
}
