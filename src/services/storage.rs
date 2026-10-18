//! Cloudflare R2 / S3 Object Storage Client Module
//!
//! Provides a dedicated, secure abstraction layer for object storage operations
//! without embedding storage infrastructure details into handlers or domain models.

use crate::config::AppConfig;
use s3::creds::Credentials;
use s3::region::Region;
use s3::Bucket;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Maximum file size limit: 5 MB per image
pub const MAX_IMAGE_SIZE_BYTES: usize = 5 * 1024 * 1024;

/// Maximum number of images allowed per product
pub const MAX_IMAGES_PER_PRODUCT: usize = 5;

/// Allowed MIME types for product images
pub const ALLOWED_MIME_TYPES: &[&str] = &["image/jpeg", "image/png", "image/webp"];

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StorageError {
    InvalidConfig(String),
    ValidationFailed(String),
    UploadFailed(String),
    DeleteFailed(String),
    InitializationFailed(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::InvalidConfig(msg) => write!(f, "Storage configuration error: {}", msg),
            StorageError::ValidationFailed(msg) => write!(f, "Image validation error: {}", msg),
            StorageError::UploadFailed(msg) => write!(f, "Storage upload failed: {}", msg),
            StorageError::DeleteFailed(msg) => write!(f, "Storage delete failed: {}", msg),
            StorageError::InitializationFailed(msg) => {
                write!(f, "Storage initialization failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for StorageError {}

/// Abstract Storage Client Trait for R2 / S3 operations
pub trait StorageClient: Send + Sync {
    fn upload_object<'a>(
        &'a self,
        key: &'a str,
        content: &'a [u8],
        content_type: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, StorageError>> + Send + 'a>>;

    fn delete_object<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), StorageError>> + Send + 'a>>;

    #[allow(dead_code)]
    fn generate_public_url(&self, key: &str) -> String;
}

pub type SharedStorageService = Arc<dyn StorageClient>;

/// Configuration model for Cloudflare R2 / S3 Storage
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StorageConfig {
    pub provider: String,
    pub bucket_name: String,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub public_url_prefix: String,
    pub max_image_size_mb: usize,
}

impl StorageConfig {
    /// Constructs storage config from AppConfig
    pub fn from_app_config(app_config: &AppConfig) -> Self {
        Self {
            provider: app_config.storage_provider.clone(),
            bucket_name: app_config.s3_bucket_name.clone(),
            endpoint: app_config.s3_endpoint.clone(),
            region: app_config.s3_region.clone(),
            access_key_id: app_config.s3_access_key_id.clone(),
            secret_access_key: app_config.s3_secret_access_key.clone(),
            public_url_prefix: app_config.storage_public_url.clone(),
            max_image_size_mb: app_config.max_image_size_mb,
        }
    }

    /// Validates that required R2 / S3 storage configuration is non-empty and valid.
    pub fn validate(&self) -> Result<(), StorageError> {
        let mut missing_fields = Vec::new();

        if self.bucket_name.trim().is_empty() {
            missing_fields.push("S3_BUCKET_NAME");
        }
        if self.endpoint.trim().is_empty() {
            missing_fields.push("S3_ENDPOINT");
        }
        if self.access_key_id.trim().is_empty() {
            missing_fields.push("S3_ACCESS_KEY_ID");
        }
        if self.secret_access_key.trim().is_empty() {
            missing_fields.push("S3_SECRET_ACCESS_KEY");
        }
        if self.public_url_prefix.trim().is_empty() {
            missing_fields.push("STORAGE_PUBLIC_URL");
        }

        if !missing_fields.is_empty() {
            return Err(StorageError::InvalidConfig(format!(
                "Missing required object storage environment variables: {}",
                missing_fields.join(", ")
            )));
        }

        Ok(())
    }
}

/// Core production Cloudflare R2 / S3 Storage Service implementation
#[allow(dead_code)]
pub struct StorageService {
    bucket: Box<Bucket>,
    public_url_prefix: String,
    config: StorageConfig,
}

impl StorageService {
    /// Instantiates a new StorageService with a verified S3/R2 Bucket client.
    /// Returns an error if configuration validation fails.
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        config.validate()?;

        let credentials = Credentials::new(
            Some(&config.access_key_id),
            Some(&config.secret_access_key),
            None,
            None,
            None,
        )
        .map_err(|e| StorageError::InitializationFailed(format!("Invalid credentials: {}", e)))?;

        let region = Region::Custom {
            region: config.region.clone(),
            endpoint: config.endpoint.clone(),
        };

        // rust-s3 0.34+ returns a `Box<Bucket>` from `Bucket::new`, and
        // `with_path_style` consumes and returns the bucket by value.
        let bucket = Bucket::new(&config.bucket_name, region, credentials)
            .map_err(|e| {
                StorageError::InitializationFailed(format!("Failed to create bucket handle: {}", e))
            })?
            .with_path_style();

        let clean_prefix = config.public_url_prefix.trim_end_matches('/').to_string();

        Ok(Self {
            bucket,
            public_url_prefix: clean_prefix,
            config,
        })
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }
}

impl StorageClient for StorageService {
    fn upload_object<'a>(
        &'a self,
        key: &'a str,
        content: &'a [u8],
        content_type: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, StorageError>> + Send + 'a>> {
        Box::pin(async move {
            if content.len() > MAX_IMAGE_SIZE_BYTES {
                return Err(StorageError::ValidationFailed(format!(
                    "File size ({} bytes) exceeds maximum limit of {} bytes (5 MB)",
                    content.len(),
                    MAX_IMAGE_SIZE_BYTES
                )));
            }

            let response = self
                .bucket
                .put_object_with_content_type(key, content, content_type)
                .await
                .map_err(|e| {
                    StorageError::UploadFailed(format!(
                        "R2 upload operation failed for key '{}': {}",
                        key, e
                    ))
                })?;

            let status = response.status_code();
            if !(200..300).contains(&status) {
                return Err(StorageError::UploadFailed(format!(
                    "R2 upload returned HTTP error status code: {}",
                    status
                )));
            }

            Ok(self.generate_public_url(key))
        })
    }

    fn delete_object<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), StorageError>> + Send + 'a>> {
        Box::pin(async move {
            let response = self.bucket.delete_object(key).await.map_err(|e| {
                StorageError::DeleteFailed(format!(
                    "R2 delete operation failed for key '{}': {}",
                    key, e
                ))
            })?;

            let status = response.status_code();
            // Treat 2xx and 404 (already absent) as success.
            if !(200..300).contains(&status) && status != 404 {
                return Err(StorageError::DeleteFailed(format!(
                    "R2 delete returned HTTP status code: {}",
                    status
                )));
            }

            Ok(())
        })
    }

    fn generate_public_url(&self, key: &str) -> String {
        let clean_key = key.trim_start_matches('/');
        format!("{}/{}", self.public_url_prefix, clean_key)
    }
}

/// Fallback implementation for unconfigured production environments.
/// Always returns an explicit StorageError::InvalidConfig when upload/delete is invoked.
pub struct UnconfiguredStorageService {
    error_message: String,
}

impl UnconfiguredStorageService {
    pub fn new(error_message: String) -> Self {
        Self { error_message }
    }
}

impl StorageClient for UnconfiguredStorageService {
    fn upload_object<'a>(
        &'a self,
        _key: &'a str,
        _content: &'a [u8],
        _content_type: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, StorageError>> + Send + 'a>> {
        let err_msg = self.error_message.clone();
        Box::pin(async move { Err(StorageError::InvalidConfig(err_msg)) })
    }

    fn delete_object<'a>(
        &'a self,
        _key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), StorageError>> + Send + 'a>> {
        let err_msg = self.error_message.clone();
        Box::pin(async move { Err(StorageError::InvalidConfig(err_msg)) })
    }

    fn generate_public_url(&self, key: &str) -> String {
        format!(
            "https://unconfigured.storage/{}",
            key.trim_start_matches('/')
        )
    }
}

/// Isolated Test-Only Storage Service implementation for unit/integration testing
#[allow(dead_code)]
pub struct TestStorageService {
    pub objects: Mutex<HashMap<String, Vec<u8>>>,
    pub public_prefix: String,
    pub fail_upload: Mutex<bool>,
    pub fail_delete: Mutex<bool>,
}

impl Default for TestStorageService {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl TestStorageService {
    pub fn new() -> Self {
        Self {
            objects: Mutex::new(HashMap::new()),
            public_prefix: "https://test-cdn.troitlogistics.com".to_string(),
            fail_upload: Mutex::new(false),
            fail_delete: Mutex::new(false),
        }
    }

    pub fn set_fail_upload(&self, should_fail: bool) {
        if let Ok(mut lock) = self.fail_upload.lock() {
            *lock = should_fail;
        }
    }

    pub fn set_fail_delete(&self, should_fail: bool) {
        if let Ok(mut lock) = self.fail_delete.lock() {
            *lock = should_fail;
        }
    }

    pub fn object_exists(&self, key: &str) -> bool {
        if let Ok(lock) = self.objects.lock() {
            lock.contains_key(key)
        } else {
            false
        }
    }
}

impl StorageClient for TestStorageService {
    fn upload_object<'a>(
        &'a self,
        key: &'a str,
        content: &'a [u8],
        _content_type: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, StorageError>> + Send + 'a>> {
        let key_owned = key.to_string();
        let content_owned = content.to_vec();
        Box::pin(async move {
            if let Ok(lock) = self.fail_upload.lock() {
                if *lock {
                    return Err(StorageError::UploadFailed(
                        "Simulated test upload failure".to_string(),
                    ));
                }
            }

            if content_owned.len() > MAX_IMAGE_SIZE_BYTES {
                return Err(StorageError::ValidationFailed(format!(
                    "File size ({} bytes) exceeds maximum limit of {} bytes (5 MB)",
                    content_owned.len(),
                    MAX_IMAGE_SIZE_BYTES
                )));
            }

            if let Ok(mut lock) = self.objects.lock() {
                lock.insert(key_owned.clone(), content_owned);
            }

            Ok(self.generate_public_url(&key_owned))
        })
    }

    fn delete_object<'a>(
        &'a self,
        key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), StorageError>> + Send + 'a>> {
        let key_owned = key.to_string();
        Box::pin(async move {
            if let Ok(lock) = self.fail_delete.lock() {
                if *lock {
                    return Err(StorageError::DeleteFailed(
                        "Simulated test delete failure".to_string(),
                    ));
                }
            }

            if let Ok(mut lock) = self.objects.lock() {
                lock.remove(&key_owned);
            }

            Ok(())
        })
    }

    fn generate_public_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_prefix, key.trim_start_matches('/'))
    }
}

/// Helper constructor to initialize StorageClient based on AppConfig
pub fn create_storage_service(config: &AppConfig) -> SharedStorageService {
    let storage_config = StorageConfig::from_app_config(config);
    match StorageService::new(storage_config) {
        Ok(svc) => Arc::new(svc),
        Err(err) => Arc::new(UnconfiguredStorageService::new(err.to_string())),
    }
}

/// Generates a standardized, secure UUID-based storage key.
/// Format: `products/{seller_id}/{product_id}/{image_uuid}.{ext}`
pub fn generate_product_image_key(seller_id: Uuid, product_id: Uuid, extension: &str) -> String {
    let clean_ext = extension.trim_start_matches('.').to_lowercase();
    let image_uuid = Uuid::new_v4();
    format!(
        "products/{}/{}/{}.{}",
        seller_id, product_id, image_uuid, clean_ext
    )
}

/// Inspects file buffer header magic bytes to verify MIME type safety.
/// Rejects files if inferred type is not in ALLOWED_MIME_TYPES.
pub fn validate_image_bytes(content: &[u8]) -> Result<String, StorageError> {
    if content.is_empty() {
        return Err(StorageError::ValidationFailed(
            "Uploaded file buffer is empty".to_string(),
        ));
    }

    if content.len() > MAX_IMAGE_SIZE_BYTES {
        return Err(StorageError::ValidationFailed(format!(
            "File size ({} bytes) exceeds max limit of 5 MB",
            content.len()
        )));
    }

    // Inspect magic bytes header using infer crate
    let kind = infer::get(content).ok_or_else(|| {
        StorageError::ValidationFailed(
            "Could not determine file type from magic bytes header".to_string(),
        )
    })?;

    let mime_type = kind.mime_type();
    if !ALLOWED_MIME_TYPES.contains(&mime_type) {
        return Err(StorageError::ValidationFailed(format!(
            "Unsupported file format '{}'. Allowed formats: JPEG, PNG, WEBP",
            mime_type
        )));
    }

    Ok(mime_type.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_key_generation_format() {
        let seller_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
        let product_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();

        let key = generate_product_image_key(seller_id, product_id, "png");

        assert!(key.starts_with(
            "products/11111111-1111-1111-1111-111111111111/22222222-2222-2222-2222-222222222222/"
        ));
        assert!(key.ends_with(".png"));
    }

    #[test]
    fn test_storage_config_validation_missing_fields() {
        let config = StorageConfig {
            provider: "r2".to_string(),
            bucket_name: "".to_string(),
            endpoint: "".to_string(),
            region: "auto".to_string(),
            access_key_id: "".to_string(),
            secret_access_key: "".to_string(),
            public_url_prefix: "".to_string(),
            max_image_size_mb: 5,
        };

        let result = config.validate();
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("S3_BUCKET_NAME"));
        assert!(err_str.contains("S3_ENDPOINT"));
        assert!(err_str.contains("S3_ACCESS_KEY_ID"));
        assert!(err_str.contains("S3_SECRET_ACCESS_KEY"));
        assert!(err_str.contains("STORAGE_PUBLIC_URL"));
    }

    #[test]
    fn test_storage_config_validation_valid() {
        let config = StorageConfig {
            provider: "r2".to_string(),
            bucket_name: "troit-bucket".to_string(),
            endpoint: "https://xxx.r2.cloudflarestorage.com".to_string(),
            region: "auto".to_string(),
            access_key_id: "test_key".to_string(),
            secret_access_key: "test_secret".to_string(),
            public_url_prefix: "https://cdn.troit.app".to_string(),
            max_image_size_mb: 5,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_magic_byte_validation_png() {
        let png_header = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        let result = validate_image_bytes(&png_header);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "image/png");
    }

    #[test]
    fn test_magic_byte_validation_jpeg() {
        let jpeg_header = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00,
        ];
        let result = validate_image_bytes(&jpeg_header);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "image/jpeg");
    }

    #[test]
    fn test_magic_byte_validation_unsupported_type() {
        let elf_header = vec![0x7F, 0x45, 0x4C, 0x46, 0x02, 0x01, 0x01, 0x00];
        let result = validate_image_bytes(&elf_header);
        assert!(result.is_err());
    }
}
