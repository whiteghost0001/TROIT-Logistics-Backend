use crate::{
    auth::service::AuthService,
    errors::AppError,
    models::{AppState, User},
    products::models::{Product, ProductResponse},
};
use serde::Serialize;
use sqlx::query_as;

pub const DEMO_SELLER_EMAIL: &str = "seller@demo.troit";
pub const DEMO_BUYER_EMAIL: &str = "buyer@demo.troit";

#[derive(Debug, Serialize)]
pub struct SeedResponse {
    pub success: bool,
    pub message: String,
    pub demo_seller_email: String,
    pub demo_buyer_email: String,
    pub seeded_products: Vec<ProductResponse>,
}

#[derive(Debug, Serialize)]
pub struct CleanupResponse {
    pub success: bool,
    pub message: String,
    pub deleted_users: u64,
    pub deleted_products: u64,
}

/// Resolve the demo password from a dev-only environment variable.
/// There is intentionally no hardcoded fallback: seeding fails unless the
/// operator explicitly provides a password via `DEMO_SEED_PASSWORD`.
fn demo_password() -> Result<String, AppError> {
    let password = std::env::var("DEMO_SEED_PASSWORD").map_err(|_| {
        AppError::Validation(
            "DEMO_SEED_PASSWORD must be set to seed demo data".to_string(),
        )
    })?;

    if password.trim().is_empty() {
        return Err(AppError::Validation(
            "DEMO_SEED_PASSWORD must not be empty".to_string(),
        ));
    }

    Ok(password)
}

/// Seeds demo users and pre-verified products.
///
/// This is NOT exposed as an HTTP route. It is only callable from the local
/// development command (see `src/bin/seed.rs` or the dev CLI entrypoint).
pub async fn seed_demo_data(state: &AppState) -> Result<SeedResponse, AppError> {
    let demo_password = demo_password()?;
    let hashed = AuthService::hash_password(&demo_password)?;

    // 1. Create or get Demo Seller
    let seller_email = DEMO_SELLER_EMAIL;
    let seller: User = match query_as::<_, User>(
        "SELECT id, email, password_hash, full_name, phone_number, role, is_active, created_at, updated_at FROM users WHERE email = $1"
    )
    .bind(seller_email)
    .fetch_optional(&state.db)
    .await? {
        Some(user) => user,
        None => {
            query_as::<_, User>(
                r#"
                INSERT INTO users (email, password_hash, full_name, phone_number, role)
                VALUES ($1, $2, 'Demo Port Harcourt Seller', '+2348012345678', 'seller')
                RETURNING id, email, password_hash, full_name, phone_number, role, is_active, created_at, updated_at
                "#
            )
            .bind(seller_email)
            .bind(&hashed)
            .fetch_one(&state.db)
            .await?
        }
    };

    // 2. Create or get Demo Buyer
    let buyer_email = DEMO_BUYER_EMAIL;
    let _buyer: User = match query_as::<_, User>(
        "SELECT id, email, password_hash, full_name, phone_number, role, is_active, created_at, updated_at FROM users WHERE email = $1"
    )
    .bind(buyer_email)
    .fetch_optional(&state.db)
    .await? {
        Some(user) => user,
        None => {
            query_as::<_, User>(
                r#"
                INSERT INTO users (email, password_hash, full_name, phone_number, role)
                VALUES ($1, $2, 'Demo Port Harcourt Buyer', '+2348098765432', 'buyer')
                RETURNING id, email, password_hash, full_name, phone_number, role, is_active, created_at, updated_at
                "#
            )
            .bind(buyer_email)
            .bind(&hashed)
            .fetch_one(&state.db)
            .await?
        }
    };

    // 3. Seed Demo Verified Products
    let demo_products_data = vec![
        (
            "iPhone 14 Pro 256GB Deep Purple",
            "Inspected Grade A like-new condition. Full original accessories, battery health 98%. Inspected by TROIT Port Harcourt Agent.",
            650000.00,
            "Grade A - Like New",
            5,
        ),
        (
            "Samsung Galaxy S23 Ultra 512GB",
            "TROIT Verified Grade A. Phantom Black, screen immaculate, physical store verified in GRA Phase 2 Port Harcourt.",
            580000.00,
            "Grade A - Certified",
            3,
        ),
        (
            "HP Spectre x360 Convertible Laptop",
            "Grade B - Excellent working condition. Intel Core i7 16GB RAM 512GB SSD. Fully tested & verified.",
            450000.00,
            "Grade B - Excellent",
            2,
        ),
    ];

    let mut seeded_products = Vec::new();

    for (name, desc, price, condition, stock) in demo_products_data {
        // Delete existing by name if any to allow fresh seed
        sqlx::query("DELETE FROM products WHERE seller_id = $1 AND name = $2")
            .bind(seller.id)
            .bind(name)
            .execute(&state.db)
            .await?;

        let product: Product = query_as::<_, Product>(
            r#"
            INSERT INTO products (seller_id, name, description, price, condition, stock, verification_status)
            VALUES ($1, $2, $3, $4, $5, $6, 'VERIFIED')
            RETURNING id, seller_id, name, description, price, condition, stock, verification_status, authenticity_status, last_inspected_at, is_african_made, african_made_category, warranty_months, warranty_terms, created_at, updated_at
            "#
        )
        .bind(seller.id)
        .bind(name)
        .bind(desc)
        .bind(price)
        .bind(condition)
        .bind(stock)
        .fetch_one(&state.db)
        .await?;

        seeded_products.push(product.to_response(vec![]));
    }

    Ok(SeedResponse {
        success: true,
        message: "Development demo data seeded successfully".to_string(),
        demo_seller_email: seller_email.to_string(),
        demo_buyer_email: buyer_email.to_string(),
        seeded_products,
    })
}

/// Removes the demo seller/buyer accounts and their demo products.
///
/// Safe to run against production databases to clean up data created by the
/// previously exposed `/api/v1/seed` endpoint.
pub async fn cleanup_demo_data(state: &AppState) -> Result<CleanupResponse, AppError> {
    let mut tx = state.db.begin().await?;

    // Delete demo products (including any VERIFIED demo products) owned by the demo seller.
    let deleted_products = sqlx::query(
        r#"
        DELETE FROM products
        WHERE seller_id IN (
            SELECT id FROM users WHERE email = ANY($1)
        )
        "#,
    )
    .bind(vec![DEMO_SELLER_EMAIL, DEMO_BUYER_EMAIL])
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let deleted_users = sqlx::query("DELETE FROM users WHERE email = ANY($1)")
        .bind(vec![DEMO_SELLER_EMAIL, DEMO_BUYER_EMAIL])
        .execute(&mut *tx)
        .await?
        .rows_affected();

    tx.commit().await?;

    Ok(CleanupResponse {
        success: true,
        message: "Demo accounts and products removed".to_string(),
        deleted_users,
        deleted_products,
    })
}
