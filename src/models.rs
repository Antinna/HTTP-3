use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ============================================================================
// ENHANCED ENUMS WITH RICH METADATA
// ============================================================================

/// User type enum with role-based access control
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum UserType {
    #[sqlx(rename = "user")]
    User,
    #[sqlx(rename = "admin")]
    Admin,
    #[sqlx(rename = "delivery_person")]
    DeliveryPerson,
}

impl UserType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
            Self::DeliveryPerson => "delivery_person",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "Customer",
            Self::Admin => "Administrator",
            Self::DeliveryPerson => "Delivery Person",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::User => "👤",
            Self::Admin => "👨‍💼",
            Self::DeliveryPerson => "🚚",
        }
    }

    pub fn permissions(&self) -> Vec<&'static str> {
        match self {
            Self::User => vec!["view_menu", "place_order", "view_own_orders"],
            Self::Admin => vec!["manage_all", "view_analytics", "manage_users", "manage_menu", "manage_orders"],
            Self::DeliveryPerson => vec!["view_assigned_orders", "update_delivery_status", "view_earnings"],
        }
    }

    pub fn all() -> Vec<UserTypeInfo> {
        vec![
            UserTypeInfo::from(&Self::User),
            UserTypeInfo::from(&Self::Admin),
            UserTypeInfo::from(&Self::DeliveryPerson),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserTypeInfo {
    pub value: String,
    pub label: String,
    pub icon: String,
    pub permissions: Vec<String>,
}

impl From<&UserType> for UserTypeInfo {
    fn from(user_type: &UserType) -> Self {
        Self {
            value: user_type.as_str().to_string(),
            label: user_type.label().to_string(),
            icon: user_type.icon().to_string(),
            permissions: user_type.permissions().iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Order status enum with progress indicators
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum OrderStatus {
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "confirmed")]
    Confirmed,
    #[sqlx(rename = "preparing")]
    Preparing,
    #[sqlx(rename = "ready_for_pickup")]
    ReadyForPickup,
    #[sqlx(rename = "out_for_delivery")]
    OutForDelivery,
    #[sqlx(rename = "delivered")]
    Delivered,
    #[sqlx(rename = "cancelled")]
    Cancelled,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Preparing => "preparing",
            Self::ReadyForPickup => "ready_for_pickup",
            Self::OutForDelivery => "out_for_delivery",
            Self::Delivered => "delivered",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Confirmed => "Confirmed",
            Self::Preparing => "Preparing",
            Self::ReadyForPickup => "Ready for Pickup",
            Self::OutForDelivery => "Out for Delivery",
            Self::Delivered => "Delivered",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "⏳",
            Self::Confirmed => "✅",
            Self::Preparing => "👨‍🍳",
            Self::ReadyForPickup => "📦",
            Self::OutForDelivery => "🚚",
            Self::Delivered => "🎉",
            Self::Cancelled => "❌",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::Pending => "#FFA500",
            Self::Confirmed => "#32CD32",
            Self::Preparing => "#1E90FF",
            Self::ReadyForPickup => "#9370DB",
            Self::OutForDelivery => "#FF6347",
            Self::Delivered => "#228B22",
            Self::Cancelled => "#DC143C",
        }
    }

    pub fn progress_percentage(&self) -> u8 {
        match self {
            Self::Pending => 10,
            Self::Confirmed => 25,
            Self::Preparing => 50,
            Self::ReadyForPickup => 75,
            Self::OutForDelivery => 90,
            Self::Delivered => 100,
            Self::Cancelled => 0,
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Delivered | Self::Cancelled)
    }

    pub fn can_cancel(&self) -> bool {
        matches!(self, Self::Pending | Self::Confirmed)
    }

    pub fn next_status(&self) -> Option<OrderStatus> {
        match self {
            Self::Pending => Some(Self::Confirmed),
            Self::Confirmed => Some(Self::Preparing),
            Self::Preparing => Some(Self::ReadyForPickup),
            Self::ReadyForPickup => Some(Self::OutForDelivery),
            Self::OutForDelivery => Some(Self::Delivered),
            Self::Delivered | Self::Cancelled => None,
        }
    }

    pub fn all() -> Vec<OrderStatusInfo> {
        vec![
            OrderStatusInfo::from(&Self::Pending),
            OrderStatusInfo::from(&Self::Confirmed),
            OrderStatusInfo::from(&Self::Preparing),
            OrderStatusInfo::from(&Self::ReadyForPickup),
            OrderStatusInfo::from(&Self::OutForDelivery),
            OrderStatusInfo::from(&Self::Delivered),
            OrderStatusInfo::from(&Self::Cancelled),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderStatusInfo {
    pub value: String,
    pub label: String,
    pub icon: String,
    pub color: String,
    pub progress_percentage: u8,
    pub is_active: bool,
    pub can_cancel: bool,
}

impl From<&OrderStatus> for OrderStatusInfo {
    fn from(status: &OrderStatus) -> Self {
        Self {
            value: status.as_str().to_string(),
            label: status.label().to_string(),
            icon: status.icon().to_string(),
            color: status.color().to_string(),
            progress_percentage: status.progress_percentage(),
            is_active: status.is_active(),
            can_cancel: status.can_cancel(),
        }
    }
}

/// Payment method enum with rich metadata
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum PaymentMethod {
    #[sqlx(rename = "cod")]
    CashOnDelivery,
    #[sqlx(rename = "upi")]
    Upi,
    #[sqlx(rename = "debit_card")]
    DebitCard,
    #[sqlx(rename = "credit_card")]
    CreditCard,
    #[sqlx(rename = "net_banking")]
    NetBanking,
    #[sqlx(rename = "digital_wallet")]
    DigitalWallet,
}

impl PaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CashOnDelivery => "cod",
            Self::Upi => "upi",
            Self::DebitCard => "debit_card",
            Self::CreditCard => "credit_card",
            Self::NetBanking => "net_banking",
            Self::DigitalWallet => "digital_wallet",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::CashOnDelivery => "Cash on Delivery",
            Self::Upi => "UPI",
            Self::DebitCard => "Debit Card",
            Self::CreditCard => "Credit Card",
            Self::NetBanking => "Net Banking",
            Self::DigitalWallet => "Digital Wallet",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::CashOnDelivery => "💵",
            Self::Upi => "📱",
            Self::DebitCard => "💳",
            Self::CreditCard => "💳",
            Self::NetBanking => "🏦",
            Self::DigitalWallet => "📲",
        }
    }

    pub fn is_online(&self) -> bool {
        !matches!(self, Self::CashOnDelivery)
    }

    pub fn processing_fee_percentage(&self) -> f64 {
        match self {
            Self::CashOnDelivery => 0.0,
            Self::Upi => 0.0,
            Self::DebitCard => 1.0,
            Self::CreditCard => 2.0,
            Self::NetBanking => 1.5,
            Self::DigitalWallet => 0.5,
        }
    }

    pub fn requires_verification(&self) -> bool {
        matches!(self, Self::DebitCard | Self::CreditCard | Self::NetBanking)
    }

    pub fn all() -> Vec<PaymentMethodInfo> {
        vec![
            PaymentMethodInfo::from(&Self::CashOnDelivery),
            PaymentMethodInfo::from(&Self::Upi),
            PaymentMethodInfo::from(&Self::DebitCard),
            PaymentMethodInfo::from(&Self::CreditCard),
            PaymentMethodInfo::from(&Self::NetBanking),
            PaymentMethodInfo::from(&Self::DigitalWallet),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentMethodInfo {
    pub value: String,
    pub label: String,
    pub icon: String,
    pub is_online: bool,
    pub processing_fee_percentage: f64,
    pub requires_verification: bool,
}

impl From<&PaymentMethod> for PaymentMethodInfo {
    fn from(method: &PaymentMethod) -> Self {
        Self {
            value: method.as_str().to_string(),
            label: method.label().to_string(),
            icon: method.icon().to_string(),
            is_online: method.is_online(),
            processing_fee_percentage: method.processing_fee_percentage(),
            requires_verification: method.requires_verification(),
        }
    }
}

/// Payment status enum with processing states
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum PaymentStatus {
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "processing")]
    Processing,
    #[sqlx(rename = "completed")]
    Completed,
    #[sqlx(rename = "failed")]
    Failed,
    #[sqlx(rename = "refunded")]
    Refunded,
    #[sqlx(rename = "partially_refunded")]
    PartiallyRefunded,
}

impl PaymentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Refunded => "refunded",
            Self::PartiallyRefunded => "partially_refunded",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Processing => "Processing",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Refunded => "Refunded",
            Self::PartiallyRefunded => "Partially Refunded",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "⏳",
            Self::Processing => "🔄",
            Self::Completed => "✅",
            Self::Failed => "❌",
            Self::Refunded => "↩️",
            Self::PartiallyRefunded => "↪️",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::Pending => "#FFA500",
            Self::Processing => "#1E90FF",
            Self::Completed => "#228B22",
            Self::Failed => "#DC143C",
            Self::Refunded => "#9370DB",
            Self::PartiallyRefunded => "#FF6347",
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Refunded)
    }

    pub fn is_successful(&self) -> bool {
        matches!(self, Self::Completed)
    }

    pub fn can_refund(&self) -> bool {
        matches!(self, Self::Completed)
    }

    pub fn all() -> Vec<PaymentStatusInfo> {
        vec![
            PaymentStatusInfo::from(&Self::Pending),
            PaymentStatusInfo::from(&Self::Processing),
            PaymentStatusInfo::from(&Self::Completed),
            PaymentStatusInfo::from(&Self::Failed),
            PaymentStatusInfo::from(&Self::Refunded),
            PaymentStatusInfo::from(&Self::PartiallyRefunded),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentStatusInfo {
    pub value: String,
    pub label: String,
    pub icon: String,
    pub color: String,
    pub is_final: bool,
    pub is_successful: bool,
    pub can_refund: bool,
}

impl From<&PaymentStatus> for PaymentStatusInfo {
    fn from(status: &PaymentStatus) -> Self {
        Self {
            value: status.as_str().to_string(),
            label: status.label().to_string(),
            icon: status.icon().to_string(),
            color: status.color().to_string(),
            is_final: status.is_final(),
            is_successful: status.is_successful(),
            can_refund: status.can_refund(),
        }
    }
}

/// Delivery status enum for delivery personnel
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "snake_case")]
pub enum DeliveryStatus {
    #[sqlx(rename = "available")]
    Available,
    #[sqlx(rename = "busy")]
    Busy,
    #[sqlx(rename = "offline")]
    Offline,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Busy => "busy",
            Self::Offline => "offline",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Available => "Available",
            Self::Busy => "Busy",
            Self::Offline => "Offline",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Available => "🟢",
            Self::Busy => "🟡",
            Self::Offline => "🔴",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::Available => "#228B22",
            Self::Busy => "#FFA500",
            Self::Offline => "#DC143C",
        }
    }

    pub fn can_assign_order(&self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn all() -> Vec<DeliveryStatusInfo> {
        vec![
            DeliveryStatusInfo::from(&Self::Available),
            DeliveryStatusInfo::from(&Self::Busy),
            DeliveryStatusInfo::from(&Self::Offline),
        ]
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeliveryStatusInfo {
    pub value: String,
    pub label: String,
    pub icon: String,
    pub color: String,
    pub can_assign_order: bool,
}

impl From<&DeliveryStatus> for DeliveryStatusInfo {
    fn from(status: &DeliveryStatus) -> Self {
        Self {
            value: status.as_str().to_string(),
            label: status.label().to_string(),
            icon: status.icon().to_string(),
            color: status.color().to_string(),
            can_assign_order: status.can_assign_order(),
        }
    }
}

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// User model with Firebase integration
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub firebase_uid: String,
    pub phone_number: String,
    pub phone_verified: bool,
    pub user_type: UserType,
    pub is_active: bool,
    pub delivery_addresses: Option<serde_json::Value>, // JSON field
    pub preferences: Option<serde_json::Value>, // JSON field
    pub email_verified_at: Option<DateTime<Utc>>,
    pub password: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Check if user has specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.user_type.permissions().contains(&permission)
    }

    /// Check if user is admin
    pub fn is_admin(&self) -> bool {
        matches!(self.user_type, UserType::Admin)
    }

    /// Check if user is delivery person
    pub fn is_delivery_person(&self) -> bool {
        matches!(self.user_type, UserType::DeliveryPerson)
    }

    /// Get user's display name
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

/// Address structure for delivery
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Address {
    pub address_type: String, // "home", "work", "other"
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub landmark: Option<String>,
    pub instructions: Option<String>,
}

/// User preferences structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserPreferences {
    pub dietary: Vec<String>, // "vegetarian", "vegan", "gluten_free", etc.
    pub spice_level: String, // "mild", "medium", "hot"
    pub notifications_enabled: bool,
    pub preferred_payment_method: Option<PaymentMethod>,
}

/// Menu item model with categories and dietary information
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct MenuItem {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub price: rust_decimal::Decimal,
    pub category: String,
    pub image_url: Option<String>,
    pub is_available: bool,
    pub is_vegetarian: bool,
    pub is_vegan: bool,
    pub ingredients: Option<serde_json::Value>, // JSON field
    pub allergens: Option<serde_json::Value>, // JSON field
    pub preparation_time: i32, // minutes
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MenuItem {
    /// Check if item matches dietary preferences
    pub fn matches_dietary_preference(&self, preference: &str) -> bool {
        match preference {
            "vegetarian" => self.is_vegetarian,
            "vegan" => self.is_vegan,
            _ => true, // Unknown preferences don't filter
        }
    }

    /// Get formatted price with currency
    pub fn formatted_price(&self, currency_symbol: &str) -> String {
        format!("{}{}", currency_symbol, self.price)
    }

    /// Check if item is available for ordering
    pub fn can_order(&self) -> bool {
        self.is_available
    }
}

/// Order model with delivery tracking
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: i64,
    pub order_number: String,
    pub user_id: i64,
    pub status: OrderStatus,
    pub delivery_address: serde_json::Value, // JSON field
    pub delivery_latitude: Option<rust_decimal::Decimal>,
    pub delivery_longitude: Option<rust_decimal::Decimal>,
    pub delivery_distance: Option<rust_decimal::Decimal>, // km
    pub subtotal: rust_decimal::Decimal,
    pub delivery_fee: rust_decimal::Decimal,
    pub tax_amount: rust_decimal::Decimal,
    pub tip_amount: Option<rust_decimal::Decimal>,
    pub total_amount: rust_decimal::Decimal,
    pub payment_status: PaymentStatus,
    pub payment_method: PaymentMethod,
    pub payment_transaction_id: Option<String>,
    pub delivery_person_id: Option<i64>,
    pub estimated_delivery_time: Option<DateTime<Utc>>,
    pub actual_delivery_time: Option<DateTime<Utc>>,
    pub special_instructions: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Generate order number with date prefix
    pub fn generate_order_number() -> String {
        let now = Utc::now();
        let date_str = now.format("%Y%m%d").to_string();
        let timestamp = now.timestamp_millis();
        format!("ORD-{}-{}", date_str, timestamp % 100000)
    }

    /// Check if order can be cancelled
    pub fn can_cancel(&self) -> bool {
        self.status.can_cancel()
    }

    /// Get order progress percentage
    pub fn progress_percentage(&self) -> u8 {
        self.status.progress_percentage()
    }

    /// Check if order is active (not delivered or cancelled)
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Get estimated delivery time remaining in minutes
    pub fn estimated_time_remaining(&self) -> Option<i64> {
        self.estimated_delivery_time.map(|est| {
            let now = Utc::now();
            if est > now {
                (est - now).num_minutes()
            } else {
                0
            }
        })
    }
}

/// Order item model for individual items within orders
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub menu_item_id: i64,
    pub quantity: i32,
    pub unit_price: rust_decimal::Decimal,
    pub total_price: rust_decimal::Decimal,
    pub customizations: Option<serde_json::Value>, // JSON field
    pub special_instructions: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrderItem {
    /// Calculate total price for the item
    pub fn calculate_total(&self) -> rust_decimal::Decimal {
        self.unit_price * rust_decimal::Decimal::from(self.quantity)
    }
}

/// Delivery personnel model with location tracking
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DeliveryPersonnel {
    pub id: i64,
    pub user_id: i64,
    pub vehicle_type: String,
    pub vehicle_number: String,
    pub license_number: String,
    pub upi_address: Option<String>,
    pub status: DeliveryStatus,
    pub current_latitude: Option<rust_decimal::Decimal>,
    pub current_longitude: Option<rust_decimal::Decimal>,
    pub last_location_update: Option<DateTime<Utc>>,
    pub rating: Option<rust_decimal::Decimal>,
    pub total_deliveries: i32,
    pub total_earnings: rust_decimal::Decimal,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DeliveryPersonnel {
    /// Check if delivery person can be assigned new orders
    pub fn can_assign_order(&self) -> bool {
        self.is_active && self.status.can_assign_order()
    }

    /// Get average rating as a formatted string
    pub fn formatted_rating(&self) -> String {
        match self.rating {
            Some(rating) => format!("{:.1} ⭐", rating),
            None => "No rating".to_string(),
        }
    }

    /// Check if location is recent (within last 10 minutes)
    pub fn has_recent_location(&self) -> bool {
        self.last_location_update
            .map(|last_update| {
                let now = Utc::now();
                (now - last_update).num_minutes() <= 10
            })
            .unwrap_or(false)
    }
}

/// Payment model for transaction records
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Payment {
    pub id: i64,
    pub order_id: i64,
    pub payment_method: PaymentMethod,
    pub payment_gateway: Option<String>,
    pub transaction_id: String,
    pub gateway_transaction_id: Option<String>,
    pub amount: rust_decimal::Decimal,
    pub status: PaymentStatus,
    pub gateway_response: Option<serde_json::Value>, // JSON field
    pub receipt_url: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Payment {
    /// Check if payment is successful
    pub fn is_successful(&self) -> bool {
        self.status.is_successful()
    }

    /// Check if payment can be refunded
    pub fn can_refund(&self) -> bool {
        self.status.can_refund()
    }

    /// Get formatted amount with currency
    pub fn formatted_amount(&self, currency_symbol: &str) -> String {
        format!("{}{}", currency_symbol, self.amount)
    }
}

/// System configuration model for dynamic settings
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SystemConfiguration {
    pub id: i64,
    pub config_key: String,
    pub config_value: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SystemConfiguration {
    /// Parse config value as boolean
    pub fn as_bool(&self) -> bool {
        matches!(self.config_value.as_str(), "1" | "true" | "yes" | "on")
    }

    /// Parse config value as integer
    pub fn as_i32(&self) -> Result<i32, std::num::ParseIntError> {
        self.config_value.parse()
    }

    /// Parse config value as float
    pub fn as_f64(&self) -> Result<f64, std::num::ParseFloatError> {
        self.config_value.parse()
    }

    /// Parse config value as decimal
    pub fn as_decimal(&self) -> Result<rust_decimal::Decimal, rust_decimal::Error> {
        self.config_value.parse()
    }
}

// ============================================================================
// HELPER STRUCTURES
// ============================================================================

/// Order summary for dashboard and analytics
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderSummary {
    pub total_orders: i64,
    pub pending_orders: i64,
    pub completed_orders: i64,
    pub cancelled_orders: i64,
    pub total_revenue: rust_decimal::Decimal,
    pub average_order_value: rust_decimal::Decimal,
}

/// Delivery metrics for performance tracking
#[derive(Debug, Serialize, Deserialize)]
pub struct DeliveryMetrics {
    pub total_deliveries: i64,
    pub average_delivery_time: i32, // minutes
    pub on_time_percentage: f64,
    pub average_rating: rust_decimal::Decimal,
    pub total_earnings: rust_decimal::Decimal,
}

/// Menu category with item count
#[derive(Debug, Serialize, Deserialize)]
pub struct MenuCategory {
    pub name: String,
    pub item_count: i64,
    pub available_count: i64,
    pub average_price: rust_decimal::Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_type_enum() {
        let user_type = UserType::Admin;
        assert_eq!(user_type.as_str(), "admin");
        assert_eq!(user_type.label(), "Administrator");
        assert_eq!(user_type.icon(), "👨‍💼");
        assert!(user_type.permissions().contains(&"manage_all"));
    }

    #[test]
    fn test_order_status_enum() {
        let status = OrderStatus::Preparing;
        assert_eq!(status.as_str(), "preparing");
        assert_eq!(status.label(), "Preparing");
        assert_eq!(status.progress_percentage(), 50);
        assert!(status.is_active());
        assert!(!status.can_cancel());
        assert_eq!(status.next_status(), Some(OrderStatus::ReadyForPickup));
    }

    #[test]
    fn test_payment_method_enum() {
        let method = PaymentMethod::Upi;
        assert_eq!(method.as_str(), "upi");
        assert_eq!(method.label(), "UPI");
        assert!(method.is_online());
        assert_eq!(method.processing_fee_percentage(), 0.0);
        assert!(!method.requires_verification());
    }

    #[test]
    fn test_payment_status_enum() {
        let status = PaymentStatus::Completed;
        assert_eq!(status.as_str(), "completed");
        assert!(status.is_final());
        assert!(status.is_successful());
        assert!(status.can_refund());
    }

    #[test]
    fn test_delivery_status_enum() {
        let status = DeliveryStatus::Available;
        assert_eq!(status.as_str(), "available");
        assert!(status.can_assign_order());
        assert_eq!(status.color(), "#228B22");
    }

    #[test]
    fn test_order_number_generation() {
        let order_number = Order::generate_order_number();
        assert!(order_number.starts_with("ORD-"));
        assert!(order_number.len() > 10);
    }

    #[test]
    fn test_system_configuration_parsing() {
        let config = SystemConfiguration {
            id: 1,
            config_key: "test_bool".to_string(),
            config_value: "true".to_string(),
            description: None,
            is_public: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        assert!(config.as_bool());
        
        let config_int = SystemConfiguration {
            id: 2,
            config_key: "test_int".to_string(),
            config_value: "42".to_string(),
            description: None,
            is_public: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        assert_eq!(config_int.as_i32().unwrap(), 42);
    }
}