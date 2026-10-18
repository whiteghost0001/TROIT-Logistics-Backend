use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Core Order entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: Uuid,
    pub buyer_id: Uuid,
    pub seller_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub amount: f64,
    pub status: String,
    pub payment_status: String,
    pub delivery_status: String,
    pub escrow_id: Option<i64>,
    pub escrow_state: String,
    pub blockchain_tx_hash: Option<String>,
    pub funding_tx_hash: Option<String>,
    pub release_tx_hash: Option<String>,
    pub refund_tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Core PickupInspection entity
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PickupInspection {
    pub id: Uuid,
    pub order_id: Uuid,
    pub inspector_id: Option<Uuid>,
    pub condition: String,
    pub notes: Option<String>,
    pub inspection_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub product_id: Uuid,
    pub quantity: Option<i32>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct FundOrderRequest {
    pub tx_hash: Option<String>,
    pub signed_tx_xdr: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct ConfirmDeliveryRequest {
    pub tx_hash: Option<String>,
    pub signed_tx_xdr: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefundOrderRequest {
    pub reason: Option<String>,
    pub tx_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DisputeOrderRequest {
    pub reason: String,
    pub tx_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveDisputeRequest {
    pub release_to_seller: bool,
    pub tx_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePickupInspectionRequest {
    pub condition: String,
    pub notes: Option<String>,
    /// Required: the inspector must explicitly state the inspection outcome.
    /// There is intentionally no default so `PASSED` can never be implied.
    pub inspection_status: String,
}

/// Audit record for a refund, capturing who requested, approved and executed it.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RefundAudit {
    pub id: Uuid,
    pub order_id: Uuid,
    pub requested_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub executed_by: Option<Uuid>,
    pub status: String,
    pub reason: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub tx_hash: Option<String>,
}

/// Approval record for a refund / dispute decision.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RefundApproval {
    pub id: Uuid,
    pub order_id: Uuid,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub id: Uuid,
    pub buyer_id: Uuid,
    pub seller_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub amount: f64,
    pub status: String,
    pub payment_status: String,
    pub delivery_status: String,
    pub escrow_id: Option<i64>,
    pub escrow_state: String,
    pub blockchain_tx_hash: Option<String>,
    pub funding_tx_hash: Option<String>,
    pub release_tx_hash: Option<String>,
    pub refund_tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    pub fn to_response(&self) -> OrderResponse {
        OrderResponse {
            id: self.id,
            buyer_id: self.buyer_id,
            seller_id: self.seller_id,
            product_id: self.product_id,
            quantity: self.quantity,
            amount: self.amount,
            status: self.status.clone(),
            payment_status: self.payment_status.clone(),
            delivery_status: self.delivery_status.clone(),
            escrow_id: self.escrow_id,
            escrow_state: self.escrow_state.clone(),
            blockchain_tx_hash: self.blockchain_tx_hash.clone(),
            funding_tx_hash: self.funding_tx_hash.clone(),
            release_tx_hash: self.release_tx_hash.clone(),
            refund_tx_hash: self.refund_tx_hash.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OrderStatusHistoryResponse {
    pub id: Uuid,
    pub order_id: Uuid,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}
