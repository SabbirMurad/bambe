use serde::{Deserialize, Serialize};

/*
  Money is stored in minor units — cents, not dollars — everywhere in this
  project. An order total is a sum of line totals, discounts and shipping, and
  f64 cannot hold 0.1 exactly, so those sums drift: three $19.99 lines add up
  to 59.969999999999999 and a cent goes missing on the invoice. Integers add
  exactly. Format for display at the edge, never in the model.
*/
pub type Cents = i64;

/*
  ISO-4217, uppercase ("USD"). The store settings hold one default and every
  priced document repeats it, so a historical order still reads correctly
  after the shop switches currency.
*/
pub type CurrencyCode = String;

pub mod metadata;
pub use metadata as Metadata;

/* ── Storefront ──────────────────────────────────────────── */

pub mod product;
pub use product as Product;

pub mod category;
pub use category as Category;

pub mod review;
pub use review as Review;

pub mod cart;
pub use cart as Cart;

/* ── Ordering ────────────────────────────────────────────── */

pub mod address;
pub use address as Address;

pub mod order;
pub use order as Order;

pub mod payment;
pub use payment as Payment;

pub mod coupon;
pub use coupon as Coupon;

/* ── Back office ─────────────────────────────────────────── */

pub mod customer;
pub use customer as Customer;

pub mod support;
pub use support as Support;

pub mod store;
pub use store as Store;

pub mod account;
pub use account as Account;

pub mod notification;
pub use notification as Notification;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AllowedImageType { Gif, Png, Jpeg, Webp }

impl std::fmt::Display for AllowedImageType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AllowedImageType::Gif => write!(fmt, "image/gif"),
            AllowedImageType::Png => write!(fmt, "image/png"),
            AllowedImageType::Jpeg => write!(fmt, "image/jpeg"),
            AllowedImageType::Webp => write!(fmt, "image/webp"),
        }
    }
}

impl AllowedImageType {
    pub fn to_str(&self) -> &str {
        match self {
            AllowedImageType::Gif => "image/gif",
            AllowedImageType::Png => "image/png",
            AllowedImageType::Jpeg => "image/jpeg",
            AllowedImageType::Webp => "image/webp",
        }
    }

    pub fn from_str(s: &str) -> AllowedImageType {
        match s {
            "image/gif" => AllowedImageType::Gif,
            "image/png" => AllowedImageType::Png,
            "image/jpeg" => AllowedImageType::Jpeg,
            "image/webp" => AllowedImageType::Webp,
            _ => AllowedImageType::Jpeg
        }
    }
}

// Where an uploaded asset ends up. The upload endpoint takes this as a string
// so a client can't dump files into the store unattributed — every image is
// claimed by something before it is written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssetUsedAt {
    ProductImage,
    CategoryImage,
    BrandLogo,
    ReviewImage,
    ReturnPhoto,
    TicketAttachment,
    ProfilePic,
    StoreLogo,
}

impl std::fmt::Display for AssetUsedAt {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt,"{:?}", self)
    }
}

impl AssetUsedAt {
    /*
      The strict form, for the upload endpoint. `from_str` below folds an
      unrecognised slot into `ProductImage`, which is fine where the value
      came from our own database but wrong at the edge — a client that
      misspells `ProfilePic` should be told, not have its avatar filed as a
      product photo.
    */
    pub fn parse(s: &str) -> Option<AssetUsedAt> {
        match s.trim() {
            "ProductImage" => Some(AssetUsedAt::ProductImage),
            "CategoryImage" => Some(AssetUsedAt::CategoryImage),
            "BrandLogo" => Some(AssetUsedAt::BrandLogo),
            "ReviewImage" => Some(AssetUsedAt::ReviewImage),
            "ReturnPhoto" => Some(AssetUsedAt::ReturnPhoto),
            "TicketAttachment" => Some(AssetUsedAt::TicketAttachment),
            "ProfilePic" => Some(AssetUsedAt::ProfilePic),
            "StoreLogo" => Some(AssetUsedAt::StoreLogo),
            _ => None,
        }
    }

    pub fn variants() -> &'static str {
        "ProductImage, CategoryImage, BrandLogo, ReviewImage, ReturnPhoto, \
         TicketAttachment, ProfilePic, StoreLogo"
    }

    pub fn from_str(s: &str) -> AssetUsedAt {
        match s {
            "ProductImage" => AssetUsedAt::ProductImage,
            "CategoryImage" => AssetUsedAt::CategoryImage,
            "BrandLogo" => AssetUsedAt::BrandLogo,
            "ReviewImage" => AssetUsedAt::ReviewImage,
            "ReturnPhoto" => AssetUsedAt::ReturnPhoto,
            "TicketAttachment" => AssetUsedAt::TicketAttachment,
            "ProfilePic" => AssetUsedAt::ProfilePic,
            "StoreLogo" => AssetUsedAt::StoreLogo,
            // Unknown values land on the safest bucket rather than failing the
            // upload, matching how the rest of these from_str helpers behave.
            _ => AssetUsedAt::ProductImage
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageStruct {
    pub uuid: String,
    pub height: usize,
    pub width: usize,
    pub original_size: usize,
    pub webp_size: usize,
    pub blur_hash: String,
    pub used_at: AssetUsedAt,
    pub original_type:  String,
    pub temporary: bool,
    pub deleted: bool,
    pub created_at: i64
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VideoStruct {
    pub uuid: String,
    pub height: usize,
    pub width: usize,
    pub thumbnail_type: AllowedImageType,
    pub length: i64,
    pub size: usize,
    pub used_at: AssetUsedAt,
    pub created_at: i64,
    pub temporary: bool,
    pub deleted: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AudioStruct {
    pub uuid: String,
    pub length: i64,
    pub size: usize,
    pub used_at: AssetUsedAt,
    pub created_at: i64,
    pub temporary: bool,
    pub deleted: bool
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AttachmentStruct {
    pub uuid: String,
    pub size: i64,
    pub name: String,
    pub extension: String,
}
