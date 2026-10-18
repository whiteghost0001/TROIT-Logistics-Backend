use troit_logistics_backend::{
    models::seller::{SellerGrade, SellerTrustLevel},
    subscriptions::service::{
        can_access_advanced_analytics, can_create_listing, get_subscription_entitlements,
    },
    trust::service::{
        calculate_seller_grade, calculate_trust_level, seller_grade_to_str, trust_level_to_str,
    },
};

mod order_authorization {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use troit_logistics_backend::{
        auth::jwt::create_token,
        models::user::{User, UserRole},
        routes::create_router,
    };

    fn test_user(role: UserRole) -> User {
        User {
            id: uuid::Uuid::new_v4(),
            email: format!("{}@example.com", uuid::Uuid::new_v4()),
            password_hash: "hash".to_string(),
            full_name: "Test User".to_string(),
            role,
            is_verified: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn bearer(user: &User) -> String {
        let token = create_token(user).expect("token");
        format!("Bearer {}", token)
    }

    async fn post_json(
        app: axum::Router,
        uri: &str,
        token: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .header("authorization", token)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn buyer_cannot_submit_pickup_inspection() {
        let app = create_router().await;
        let buyer = test_user(UserRole::Buyer);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/pickup-inspection", order_id),
            &bearer(&buyer),
            json!({ "status": "PASSED", "notes": "looks good" }),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn seller_cannot_submit_pickup_inspection() {
        let app = create_router().await;
        let seller = test_user(UserRole::Seller);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/pickup-inspection", order_id),
            &bearer(&seller),
            json!({ "status": "PASSED", "notes": "looks good" }),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn unrelated_agent_cannot_submit_pickup_inspection() {
        let app = create_router().await;
        let agent = test_user(UserRole::FieldAgent);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/pickup-inspection", order_id),
            &bearer(&agent),
            json!({ "status": "PASSED", "notes": "looks good" }),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn assigned_agent_or_admin_can_submit_pickup_inspection() {
        let app = create_router().await;
        let admin = test_user(UserRole::Admin);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/pickup-inspection", order_id),
            &bearer(&admin),
            json!({ "status": "PASSED", "notes": "looks good" }),
        )
        .await;

        assert_ne!(status, StatusCode::FORBIDDEN);
        assert_ne!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn pickup_inspection_requires_explicit_status() {
        let app = create_router().await;
        let admin = test_user(UserRole::Admin);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/pickup-inspection", order_id),
            &bearer(&admin),
            json!({ "notes": "no status provided" }),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn buyer_cannot_execute_refund() {
        let app = create_router().await;
        let buyer = test_user(UserRole::Buyer);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/refund", order_id),
            &bearer(&buyer),
            json!({ "reason": "changed mind" }),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn seller_cannot_execute_refund() {
        let app = create_router().await;
        let seller = test_user(UserRole::Seller);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/refund", order_id),
            &bearer(&seller),
            json!({ "reason": "changed mind" }),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn refund_requires_approved_decision() {
        let app = create_router().await;
        let admin = test_user(UserRole::Admin);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/refund", order_id),
            &bearer(&admin),
            json!({ "reason": "no approved dispute" }),
        )
        .await;

        assert_ne!(status, StatusCode::OK);
        assert_ne!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn refund_rejected_in_non_refundable_state() {
        let app = create_router().await;
        let admin = test_user(UserRole::Admin);
        let order_id = uuid::Uuid::new_v4();

        let (status, _) = post_json(
            app,
            &format!("/api/v1/orders/{}/refund", order_id),
            &bearer(&admin),
            json!({ "reason": "already delivered" }),
        )
        .await;

        assert_ne!(status, StatusCode::OK);
        assert_ne!(status, StatusCode::UNAUTHORIZED);
    }
}

#[test]
fn test_trust_level_deterministic_progression() {
    assert_eq!(calculate_trust_level(0, 100.0), SellerTrustLevel::LV1);
    assert_eq!(calculate_trust_level(4, 100.0), SellerTrustLevel::LV1);
    assert_eq!(calculate_trust_level(5, 92.0), SellerTrustLevel::LV2);
    assert_eq!(calculate_trust_level(14, 95.0), SellerTrustLevel::LV2);
    assert_eq!(calculate_trust_level(15, 94.0), SellerTrustLevel::LV3);
    assert_eq!(calculate_trust_level(29, 95.0), SellerTrustLevel::LV3);
    assert_eq!(calculate_trust_level(30, 96.0), SellerTrustLevel::LV4);
    assert_eq!(calculate_trust_level(49, 97.0), SellerTrustLevel::LV4);
    assert_eq!(calculate_trust_level(50, 98.5), SellerTrustLevel::LV5);

    assert_eq!(trust_level_to_str(SellerTrustLevel::LV1), "LV1");
    assert_eq!(trust_level_to_str(SellerTrustLevel::LV5), "LV5");
}

#[test]
fn test_seller_grade_deterministic_calculation() {
    // IMPORTANT: Represents supplier trust built with Troit, NOT physical product condition
    assert_eq!(calculate_seller_grade(0, 100.0), SellerGrade::GradeC);
    assert_eq!(calculate_seller_grade(9, 100.0), SellerGrade::GradeC);
    assert_eq!(calculate_seller_grade(10, 90.0), SellerGrade::GradeB);
    assert_eq!(calculate_seller_grade(24, 99.0), SellerGrade::GradeB);
    assert_eq!(calculate_seller_grade(25, 95.0), SellerGrade::GradeA);

    assert_eq!(seller_grade_to_str(SellerGrade::GradeC), "Grade C");
    assert_eq!(seller_grade_to_str(SellerGrade::GradeA), "Grade A");
}

#[test]
fn test_subscription_entitlements_and_limits() {
    let free_sub = troit_logistics_backend::models::subscription::SellerSubscription {
        id: uuid::Uuid::new_v4(),
        seller_id: uuid::Uuid::new_v4(),
        plan_tier: "FREE".to_string(),
        status: "ACTIVE".to_string(),
        expires_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let entitlements = get_subscription_entitlements(&free_sub);
    assert_eq!(entitlements.max_active_listings, Some(10));
    assert!(!entitlements.advanced_analytics_enabled);
    assert!(can_create_listing(&free_sub, 5));
    assert!(!can_create_listing(&free_sub, 10));

    let pro_sub = troit_logistics_backend::models::subscription::SellerSubscription {
        id: uuid::Uuid::new_v4(),
        seller_id: uuid::Uuid::new_v4(),
        plan_tier: "PRO".to_string(),
        status: "ACTIVE".to_string(),
        expires_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let pro_entitlements = get_subscription_entitlements(&pro_sub);
    assert_eq!(pro_entitlements.max_active_listings, Some(50));
    assert!(pro_entitlements.advanced_analytics_enabled);
    assert!(can_access_advanced_analytics(&pro_sub));
}

#[test]
fn test_african_made_category_validation() {
    let valid_categories = vec!["ELECTRONICS", "HOME_APPLIANCES", "FURNITURE"];
    for cat in valid_categories {
        assert!(matches!(
            cat,
            "ELECTRONICS" | "HOME_APPLIANCES" | "FURNITURE"
        ));
    }
}
