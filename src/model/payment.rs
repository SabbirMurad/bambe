use serde::{Deserialize, Serialize};
use super::{Cents, CurrencyCode};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PaymentMethodKind {
    Card,
    PayPal,
    ApplePay,
    GooglePay,
    CashOnDelivery,
}
impl std::fmt::Display for PaymentMethodKind {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CardBrand {
    Visa,
    Mastercard,
    Amex,
    Discover,
    Unknown,
}
impl std::fmt::Display for CardBrand {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  payment_method — a saved way to pay.

  There is no card number here, and there must never be one. What is stored is
  the provider's token plus the handful of fields needed to *draw* the saved
  card in the UI: brand, last four, expiry. Holding the PAN or the CVV would
  drag this project into PCI-DSS scope for no benefit the token doesn't
  already give.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentMethod {
    pub uuid: String,
    pub user_id: String,
    pub kind: PaymentMethodKind,

    // Opaque handle from the processor — the only thing that can charge.
    pub provider: String,                // "stripe" | "paypal" | ...
    pub provider_token: String,

    // Display only.
    pub card_brand: Option<CardBrand>,
    pub last4: Option<String>,
    pub exp_month: Option<u8>,
    pub exp_year: Option<u16>,
    pub holder_name: Option<String>,
    // For a PayPal account, the address it belongs to.
    pub account_email: Option<String>,

    pub is_default: bool,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionKind {
    Authorisation,
    Capture,
    Sale,
    Refund,
    Void,
}
impl std::fmt::Display for TransactionKind {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TransactionStatus {
    #[default]
    Pending,
    Succeeded,
    Failed,
}
impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/*
  payment_transaction — append-only record of every call to the processor.

  One order can produce several: an authorisation, a capture, a partial refund
  later. The order carries a single summarised `payment_status`; this is where
  the actual money movement is reconstructed from when the two disagree.

  Put a unique index on `provider_transaction_id`. Payment webhooks are
  delivered more than once by design, and that index is what stops a retry from
  refunding a customer twice.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaymentTransaction {
    pub uuid: String,
    pub order_id: String,
    pub user_id: String,

    pub kind: TransactionKind,
    pub status: TransactionStatus,
    pub amount: Cents,
    pub currency: CurrencyCode,

    pub provider: String,
    // Dedupe key — unique.
    pub provider_transaction_id: String,
    pub method_kind: PaymentMethodKind,
    // Which saved method was used, when one was. None for a one-off card.
    pub payment_method_id: Option<String>,

    pub failure_code: Option<String>,
    pub failure_message: Option<String>,

    pub created_at: i64,
    pub settled_at: Option<i64>,
}
