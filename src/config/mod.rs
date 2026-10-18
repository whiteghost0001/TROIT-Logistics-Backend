use std::env;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppConfig {
    pub database_url: String,
    pub app_host: String,
    pub app_port: u16,
    pub rust_log: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub stellar_rpc_url: String,
    pub stellar_network_passphrase: String,
    pub soroban_escrow_contract_id: String,
    pub soroban_token_contract_id: String,
    pub soroban_admin_secret_key: String,
    pub soroban_service_secret_key: String,
    pub soroban_buyer_secret_key: String,
    pub soroban_seller_secret_key: String,
    pub soroban_max_fee: u64,
    pub storage_provider: String,
    pub s3_bucket_name: String,
    pub s3_endpoint: String,
    pub s3_region: String,
    pub s3_access_key_id: String,
    pub s3_secret_access_key: String,
    pub storage_public_url: String,
    pub max_image_size_mb: usize,
    pub cors_allowed_origins: Vec<String>,
    pub admin_bootstrap_secret: Option<String>,
    /// Deployment environment, e.g. "production", "staging", "development".
    /// Defaults to "production" so that development-only features (such as the
    /// demo seed command) are never enabled by accident.
    pub app_env: String,
}

impl AppConfig {
    /// Returns true only when the app is explicitly running in development.
    pub fn is_development(&self) -> bool {
        matches!(
            self.app_env.trim().to_ascii_lowercase().as_str(),
            "development" | "dev" | "local"
        )
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        // Attempt to load .env file if available
        let _ = dotenvy::dotenv();

        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL environment variable must be set".to_string())?;

        let app_host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let app_port = env::var("PORT")
            .or_else(|_| env::var("APP_PORT"))
            .unwrap_or_else(|_| "8000".to_string())
            .parse::<u16>()
            .map_err(|e| format!("Invalid port configuration: {}", e))?;

        let rust_log = env::var("RUST_LOG")
            .unwrap_or_else(|_| "info,troit_logistics_backend=debug".to_string());

        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET environment variable must be set".to_string())?;

        let jwt_expiration_hours = env::var("JWT_EXPIRATION_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse::<i64>()
            .map_err(|e| format!("Invalid JWT_EXPIRATION_HOURS configuration: {}", e))?;

        let stellar_rpc_url = env::var("STELLAR_RPC_URL")
            .unwrap_or_else(|_| "https://soroban-testnet.stellar.org".to_string());

        let stellar_network_passphrase = env::var("STELLAR_NETWORK_PASSPHRASE")
            .unwrap_or_else(|_| "Test SDF Network ; September 2015".to_string());

        let soroban_escrow_contract_id =
            env::var("SOROBAN_ESCROW_CONTRACT_ID").unwrap_or_else(|_| {
                "CBJB3R7RZSXXA5IDZRDMXEBRTWEZKRUSEVIA5C3M5D7F4H3QDM4MJ42P".to_string()
            });

        // Testnet native XLM Stellar Asset Contract, used as the escrow's payment
        // token unless overridden. Verified against the deployed passphrase above.
        let soroban_token_contract_id =
            env::var("SOROBAN_TOKEN_CONTRACT_ID").unwrap_or_else(|_| {
                "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC".to_string()
            });

        let soroban_admin_secret_key = env::var("SOROBAN_ADMIN_SECRET_KEY").unwrap_or_default();
        let soroban_service_secret_key = env::var("SOROBAN_SERVICE_SECRET_KEY").unwrap_or_default();
        let soroban_buyer_secret_key = env::var("SOROBAN_BUYER_SECRET_KEY").unwrap_or_default();
        let soroban_seller_secret_key = env::var("SOROBAN_SELLER_SECRET_KEY").unwrap_or_default();

        let soroban_max_fee = env::var("SOROBAN_MAX_FEE")
            .unwrap_or_else(|_| "5000000".to_string())
            .parse::<u64>()
            .map_err(|e| format!("Invalid SOROBAN_MAX_FEE configuration: {}", e))?;

        let storage_provider = env::var("STORAGE_PROVIDER").unwrap_or_else(|_| "r2".to_string());
        let s3_bucket_name = env::var("S3_BUCKET_NAME").unwrap_or_default();
        let s3_endpoint = env::var("S3_ENDPOINT").unwrap_or_default();
        let s3_region = env::var("S3_REGION").unwrap_or_else(|_| "auto".to_string());
        let s3_access_key_id = env::var("S3_ACCESS_KEY_ID").unwrap_or_default();
        let s3_secret_access_key = env::var("S3_SECRET_ACCESS_KEY").unwrap_or_default();
        let storage_public_url = env::var("STORAGE_PUBLIC_URL").unwrap_or_default();

        let max_image_size_mb = env::var("MAX_IMAGE_SIZE_MB")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<usize>()
            .map_err(|e| format!("Invalid MAX_IMAGE_SIZE_MB configuration: {}", e))?;

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .or_else(|_| env::var("FRONTEND_URL"))
            .unwrap_or_else(|_| {
                "https://troit-logistics.vercel.app,http://localhost:3000,http://localhost:5173,http://127.0.0.1:3000,http://127.0.0.1:5173".to_string()
            })
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<String>>();

        let admin_bootstrap_secret = env::var("ADMIN_BOOTSTRAP_SECRET")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        // Default to production: development-only features must be opted into.
        let app_env = env::var("APP_ENV")
            .or_else(|_| env::var("ENVIRONMENT"))
            .unwrap_or_else(|_| "production".to_string())
            .trim()
            .to_ascii_lowercase();

        Ok(Self {
            database_url,
            app_host,
            app_port,
            rust_log,
            jwt_secret,
            jwt_expiration_hours,
            stellar_rpc_url,
            stellar_network_passphrase,
            soroban_escrow_contract_id,
            soroban_token_contract_id,
            soroban_admin_secret_key,
            soroban_service_secret_key,
            soroban_buyer_secret_key,
            soroban_seller_secret_key,
            soroban_max_fee,
            storage_provider,
            s3_bucket_name,
            s3_endpoint,
            s3_region,
            s3_access_key_id,
            s3_secret_access_key,
            storage_public_url,
            max_image_size_mb,
            cors_allowed_origins,
            admin_bootstrap_secret,
            app_env,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_app_env_defaults_to_production() {
        let orig_app_env = env::var("APP_ENV").ok();
        let orig_environment = env::var("ENVIRONMENT").ok();
        let orig_database_url = env::var("DATABASE_URL").ok();
        let orig_jwt_secret = env::var("JWT_SECRET").ok();

        env::set_var("DATABASE_URL", "postgres://test:test@localhost:5432/test");
        env::set_var("JWT_SECRET", "test_jwt_secret_for_unit_tests_only");
        env::remove_var("APP_ENV");
        env::remove_var("ENVIRONMENT");

        let config = AppConfig::from_env().expect("Config loading failed");
        assert_eq!(config.app_env, "production");
        assert!(!config.is_development());

        env::set_var("APP_ENV", "development");
        let config = AppConfig::from_env().expect("Config loading failed");
        assert!(config.is_development());

        if let Some(val) = orig_app_env {
            env::set_var("APP_ENV", val);
        } else {
            env::remove_var("APP_ENV");
        }
        if let Some(val) = orig_environment {
            env::set_var("ENVIRONMENT", val);
        } else {
            env::remove_var("ENVIRONMENT");
        }
        if let Some(val) = orig_database_url {
            env::set_var("DATABASE_URL", val);
        } else {
            env::remove_var("DATABASE_URL");
        }
        if let Some(val) = orig_jwt_secret {
            env::set_var("JWT_SECRET", val);
        } else {
            env::remove_var("JWT_SECRET");
        }
    }

    #[test]
    fn test_port_resolution_priority() {
        let orig_port = env::var("PORT").ok();
        let orig_app_port = env::var("APP_PORT").ok();
        let orig_database_url = env::var("DATABASE_URL").ok();
        let orig_jwt_secret = env::var("JWT_SECRET").ok();

        // Required configuration must be present for from_env() to succeed.
        env::set_var("DATABASE_URL", "postgres://test:test@localhost:5432/test");
        env::set_var("JWT_SECRET", "test_jwt_secret_for_unit_tests_only");

        // 1. PORT takes priority over APP_PORT and default
        env::set_var("PORT", "9000");
        env::set_var("APP_PORT", "7000");
        let config = AppConfig::from_env().expect("Config loading failed");
        assert_eq!(config.app_port, 9000);

        // 2. APP_PORT takes priority when PORT is not set
        env::remove_var("PORT");
        env::set_var("APP_PORT", "7000");
        let config = AppConfig::from_env().expect("Config loading failed");
        assert_eq!(config.app_port, 7000);

        // 3. Default 8000 is used when neither PORT nor APP_PORT is set
        env::remove_var("PORT");
        env::remove_var("APP_PORT");
        let config = AppConfig::from_env().expect("Config loading failed");
        assert_eq!(config.app_port, 8000);

        // Clean up environment variables
        if let Some(val) = orig_port {
            env::set_var("PORT", val);
        } else {
            env::remove_var("PORT");
        }
        if let Some(val) = orig_app_port {
            env::set_var("APP_PORT", val);
        } else {
            env::remove_var("APP_PORT");
        }
        if let Some(val) = orig_database_url {
            env::set_var("DATABASE_URL", val);
        } else {
            env::remove_var("DATABASE_URL");
        }
        if let Some(val) = orig_jwt_secret {
            env::set_var("JWT_SECRET", val);
        } else {
            env::remove_var("JWT_SECRET");
        }
    }
}
