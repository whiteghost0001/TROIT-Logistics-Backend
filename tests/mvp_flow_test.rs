#[cfg(test)]
mod tests {
    use troit_logistics_backend::{
        auth::service::AuthService,
        config::AppConfig,
        models::UserRole,
        products::models::VerificationStatus,
        routes,
        utils::{validate_email, validate_password},
    };
    use uuid::Uuid;

    fn production_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.environment = "production".to_string();
        config.enable_seed_endpoint = false;
        config
    }

    fn development_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.environment = "development".to_string();
        config.enable_seed_endpoint = true;
        config
    }

    #[test]
    fn test_seed_route_unavailable_in_production() {
        let router = routes::build_router(production_config());
        assert!(
            !routes::seed_route_registered(&router),
            "POST /api/v1/seed must not be registered in production"
        );
    }

    #[test]
    fn test_seed_route_available_only_in_development() {
        let router = routes::build_router(development_config());
        assert!(
            routes::seed_route_registered(&router),
            "POST /api/v1/seed should be registered when the development flag is enabled"
        );
    }

    #[test]
    fn test_verification_status_conversions() {
        assert_eq!(VerificationStatus::Pending.as_str(), "PENDING");
        assert_eq!(VerificationStatus::Verified.as_str(), "VERIFIED");
        assert_eq!(VerificationStatus::Rejected.as_str(), "REJECTED");

        assert_eq!(
            VerificationStatus::from_str("VERIFIED"),
            VerificationStatus::Verified
        );
        assert_eq!(
            VerificationStatus::from_str("REJECTED"),
            VerificationStatus::Rejected
        );
        assert_eq!(
            VerificationStatus::from_str("UNKNOWN"),
            VerificationStatus::Pending
        );
    }

    #[test]
    fn test_auth_and_user_roles() {
        let seller_id = Uuid::new_v4();
        let buyer_id = Uuid::new_v4();
        let secret = "troit_jwt_demo_secret_key_123456";

        let seller_token = AuthService::generate_token(
            seller_id,
            "seller@troit.test",
            UserRole::Seller,
            secret,
            24,
        )
        .expect("Seller token generation failed");
        let buyer_token =
            AuthService::generate_token(buyer_id, "buyer@troit.test", UserRole::Buyer, secret, 24)
                .expect("Buyer token generation failed");

        let seller_claims = AuthService::verify_token(&seller_token, secret)
            .expect("Seller token verification failed");
        let buyer_claims = AuthService::verify_token(&buyer_token, secret)
            .expect("Buyer token verification failed");

        assert_eq!(seller_claims.role, UserRole::Seller);
        assert_eq!(buyer_claims.role, UserRole::Buyer);
        assert_ne!(seller_claims.sub, buyer_claims.sub);
    }

    #[test]
    fn test_input_validation_rules() {
        assert!(validate_email("seller@port-harcourt.ng").is_ok());
        assert!(validate_email("invalid").is_err());
        assert!(validate_password("DemoPass123!").is_ok());
        assert!(validate_password("short").is_err());
    }
}
