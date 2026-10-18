//! Phase 5F Step 2: Storage Client & Database Migration Test Suite
//!
//! Validates:
//! 1. Storage configuration loading and strict validation rules (Requirement 8).
//! 2. UUID-based storage key format generation.
//! 3. Binary magic byte inspection & MIME type validation for image uploads.
//! 4. Database schema migration for product_images, including foreign key relationships and ON DELETE CASCADE.

use troit_logistics_backend::services::storage::{
    generate_product_image_key, validate_image_bytes, StorageConfig, StorageError, StorageService,
    ALLOWED_MIME_TYPES, MAX_IMAGES_PER_PRODUCT, MAX_IMAGE_SIZE_BYTES,
};
use uuid::Uuid;

fn valid_test_config() -> StorageConfig {
    StorageConfig {
        provider: "r2".to_string(),
        bucket_name: "troit-prod-bucket".to_string(),
        endpoint: "https://account_id.r2.cloudflarestorage.com".to_string(),
        region: "auto".to_string(),
        access_key_id: "valid_access_key".to_string(),
        secret_access_key: "valid_secret_key".to_string(),
        public_url_prefix: "https://cdn.troitlogistics.com".to_string(),
        max_image_size_mb: 5,
    }
}

#[test]
fn test_storage_config_strict_validation() {
    let empty_config = StorageConfig {
        provider: "r2".to_string(),
        bucket_name: "".to_string(),
        endpoint: "".to_string(),
        region: "auto".to_string(),
        access_key_id: "".to_string(),
        secret_access_key: "".to_string(),
        public_url_prefix: "".to_string(),
        max_image_size_mb: 5,
    };

    let validation_result = empty_config.validate();
    assert!(validation_result.is_err());

    if let Err(StorageError::InvalidConfig(msg)) = validation_result {
        assert!(msg.contains("S3_BUCKET_NAME"));
        assert!(msg.contains("S3_ENDPOINT"));
        assert!(msg.contains("S3_ACCESS_KEY_ID"));
        assert!(msg.contains("S3_SECRET_ACCESS_KEY"));
        assert!(msg.contains("STORAGE_PUBLIC_URL"));
    } else {
        panic!("Expected StorageError::InvalidConfig");
    }
}

#[test]
fn test_storage_config_valid_pass() {
    let valid_config = valid_test_config();

    assert!(valid_config.validate().is_ok());
}

#[test]
fn test_storage_service_construction_with_upgraded_s3_client() {
    // Constructing the service exercises the rust-s3 client builder against the
    // upgraded dependency; no network calls are made here.
    let service = StorageService::new(valid_test_config())
        .expect("StorageService must construct from a valid config");
    assert!(service.is_ok());
}

#[test]
fn test_storage_service_rejects_invalid_config() {
    let mut config = valid_test_config();
    config.bucket_name = "".to_string();

    let result = StorageService::new(config);
    assert!(result.is_err());
    match result {
        Err(StorageError::InvalidConfig(_)) => {}
        _ => panic!("Expected StorageError::InvalidConfig"),
    }
}

#[test]
fn test_storage_service_public_url_generation() {
    let service = StorageService::new(valid_test_config())
        .expect("StorageService must construct from a valid config");

    let seller_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let key = generate_product_image_key(seller_id, product_id, "png");

    let url = service.generate_public_url(&key);
    assert_eq!(
        url,
        format!("https://cdn.troitlogistics.com/{}", key),
        "Public URL must be prefixed with the configured CDN prefix"
    );
}

#[tokio::test]
async fn test_storage_service_upload_and_delete_api_surface() {
    // The upgraded rust-s3 client exposes upload/delete through the service.
    // Without live R2 credentials these calls must fail gracefully (returning an
    // error) rather than panicking, which validates the new API wiring.
    let service = StorageService::new(valid_test_config())
        .expect("StorageService must construct from a valid config");

    let seller_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();
    let key = generate_product_image_key(seller_id, product_id, "png");

    let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];

    let upload_result = service.upload_object(&key, png_bytes, "image/png").await;
    assert!(
        upload_result.is_err(),
        "Upload against an unreachable endpoint must return an error, not panic"
    );

    let delete_result = service.delete_object(&key).await;
    assert!(
        delete_result.is_err(),
        "Delete against an unreachable endpoint must return an error, not panic"
    );
}

#[test]
fn test_storage_key_generation_uuid_format() {
    let seller_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let key_png = generate_product_image_key(seller_id, product_id, "png");
    assert!(key_png.starts_with(&format!("products/{}/{}/", seller_id, product_id)));
    assert!(key_png.ends_with(".png"));

    let key_webp = generate_product_image_key(seller_id, product_id, ".WEBP");
    assert!(key_webp.starts_with(&format!("products/{}/{}/", seller_id, product_id)));
    assert!(key_webp.ends_with(".webp"));
}

#[test]
fn test_image_limits_constants() {
    assert_eq!(MAX_IMAGE_SIZE_BYTES, 5 * 1024 * 1024);
    assert_eq!(MAX_IMAGES_PER_PRODUCT, 5);
    assert_eq!(ALLOWED_MIME_TYPES.len(), 3);
    assert!(ALLOWED_MIME_TYPES.contains(&"image/jpeg"));
    assert!(ALLOWED_MIME_TYPES.contains(&"image/png"));
    assert!(ALLOWED_MIME_TYPES.contains(&"image/webp"));
}

#[test]
fn test_image_magic_byte_validation() {
    // Valid PNG Magic Header: \x89PNG\r\n\x1a\n
    let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
    let png_res = validate_image_bytes(&png_bytes);
    assert!(png_res.is_ok());
    assert_eq!(png_res.unwrap(), "image/png");

    // Valid JPEG Magic Header: \xFF\xD8\xFF\xE0
    let jpeg_bytes = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00,
    ];
    let jpeg_res = validate_image_bytes(&jpeg_bytes);
    assert!(jpeg_res.is_ok());
    assert_eq!(jpeg_res.unwrap(), "image/jpeg");

    // Invalid / Malicious binary (HTML script disguised as text/binary)
    let script_bytes = b"<script>alert('xss')</script>".to_vec();
    let script_res = validate_image_bytes(&script_bytes);
    assert!(script_res.is_err());
}

#[tokio::test]
async fn test_database_product_images_migration_and_cascade() {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/troit_logistics".to_string()
    });

    let pool = match sqlx::PgPool::connect(&database_url).await {
        Ok(p) => p,
        Err(_) => {
            println!(
                "Skipping DB integration test: Postgres connection unavailable at {}",
                database_url
            );
            return;
        }
    };

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("SQLx migrations must execute cleanly");

    // 1. Create temporary seller user
    let seller_id = Uuid::new_v4();
    let email = format!("test_seller_{}@example.com", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, full_name, role) VALUES ($1, $2, 'hash', 'Test Seller', 'seller')",
    )
    .bind(seller_id)
    .bind(&email)
    .execute(&pool)
    .await
    .expect("Failed to insert test seller user");

    // 2. Create test product
    let product_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO products (id, seller_id, name, description, price) VALUES ($1, $2, 'Test Phone', 'Test Desc', 100.0)",
    )
    .bind(product_id)
    .bind(seller_id)
    .execute(&pool)
    .await
    .expect("Failed to insert test product");

    // 3. Insert product image record
    let image_id = Uuid::new_v4();
    let storage_key = format!("products/{}/{}/{}.png", seller_id, product_id, image_id);
    let public_url = format!("https://cdn.troitlogistics.com/{}", storage_key);

    sqlx::query(
        r#"
        INSERT INTO product_images (id, product_id, url, storage_key, mime_type, file_size, sort_order)
        VALUES ($1, $2, $3, $4, 'image/png', 1024, 0)
        "#,
    )
    .bind(image_id)
    .bind(product_id)
    .bind(&public_url)
    .bind(&storage_key)
    .execute(&pool)
    .await
    .expect("Failed to insert product image record");

    // 4. Verify image record exists
    let row = sqlx::query(
        "SELECT id, product_id, url, storage_key, mime_type, file_size, sort_order FROM product_images WHERE id = $1",
    )
    .bind(image_id)
    .fetch_one(&pool)
    .await
    .expect("Product image record must exist");

    use sqlx::Row;
    let fetched_pid: Uuid = row.get("product_id");
    let fetched_url: String = row.get("url");
    let fetched_key: String = row.get("storage_key");
    let fetched_mime: String = row.get("mime_type");
    let fetched_size: i32 = row.get("file_size");
    let fetched_sort: i32 = row.get("sort_order");

    assert_eq!(fetched_pid, product_id);
    assert_eq!(fetched_url, public_url);
    assert_eq!(fetched_key, storage_key);
    assert_eq!(fetched_mime, "image/png");
    assert_eq!(fetched_size, 1024);
    assert_eq!(fetched_sort, 0);

    // 5. Test ON DELETE CASCADE behavior: Delete product and verify image row is deleted automatically
    sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(product_id)
        .execute(&pool)
        .await
        .expect("Failed to delete test product");

    let deleted_img = sqlx::query("SELECT id FROM product_images WHERE id = $1")
        .bind(image_id)
        .fetch_optional(&pool)
        .await
        .expect("Query failed");

    assert!(
        deleted_img.is_none(),
        "Product image record must be deleted via ON DELETE CASCADE"
    );

    // Clean up test seller
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(seller_id)
        .execute(&pool)
        .await;
}
