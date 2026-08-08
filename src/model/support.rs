use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TicketStatus {
    #[default]
    Open,
    // Staff have replied; waiting on the customer.
    Pending,
    Resolved,
    Closed,
}
impl std::fmt::Display for TicketStatus {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

// The topic chips in the support inbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TicketTopic {
    Shipping,
    Returns,
    Billing,
    Stock,
    Damage,
    Account,
    Promotions,
    Other,
}
impl std::fmt::Display for TicketTopic {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TicketPriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}
impl std::fmt::Display for TicketPriority {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

//support_ticket
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SupportTicket {
    pub uuid: String,
    // The reference the customer quotes — "TK-2035".
    pub number: String,

    // None for a guest writing in about an order they placed without an
    // account; `email` is then the only way back to them.
    pub user_id: Option<String>,
    pub email: String,
    pub name: String,

    pub subject: String,
    pub topic: TicketTopic,
    pub status: TicketStatus,
    pub priority: TicketPriority,
    // Which order it is about, when it is about one.
    pub order_id: Option<String>,
    pub assigned_to: Option<String>,

    /*
      Denormalised for the inbox list, which shows a preview line and a
      timestamp per ticket. Without these the list would open every thread to
      draw itself.
    */
    pub last_message_preview: String,
    pub last_message_at: i64,
    pub unread_by_staff: bool,
    pub message_count: u32,

    pub created_at: i64,
    pub resolved_at: Option<i64>,
    pub closed_at: Option<i64>,
}

/*
  ticket_message — one turn in the thread, in its own collection.

  Not embedded in the ticket: threads grow without limit, and every reply would
  rewrite the whole document. Messages are also read on their own — searching
  support history — which an embedded array makes awkward.
*/
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TicketMessage {
    pub uuid: String,
    pub ticket_id: String,

    // True when staff wrote it. Not derived from user_id: an admin can hold a
    // customer account on the same shop.
    pub from_staff: bool,
    pub author_id: Option<String>,
    pub author_name: String,

    pub body: String,
    pub image_ids: Vec<String>,
    // An internal note staff leave on the thread, never sent to the customer.
    pub is_internal: bool,

    pub created_at: i64,
}
