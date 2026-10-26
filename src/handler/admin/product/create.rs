use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Cents;
use crate::model::Product::{Product, ProductOption, ProductVariant, ProductStatus, VariantOption};

#[derive(Debug, Deserialize)]
pub struct VariantInput {
    sku: String,
    /// One value per product option, in the product's option order.
    #[serde(default)]
    options: Vec<String>,
    color_hex: Option<String>,
    image_id: String,
    #[serde(default)]
    extra_image_ids: Vec<String>,
    price: Cents,
    compare_at_price: Option<Cents>,
    #[serde(default)]
    stock: i32,
    #[serde(default = "default_low_stock")]
    low_stock_threshold: i32,
}
fn default_low_stock() -> i32 { 5 }

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    title: String,
    slug: Option<String>,
    description: String,
    brand_id: Option<String>,
    category_id: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    status: ProductStatus,
    #[serde(default)]
    options: Vec<ProductOption>,
    #[serde(default)]
    variants: Vec<VariantInput>,
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in input.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    if body.title.trim().is_empty() || body.category_id.trim().is_empty() {
        return Ok(Response::bad_request("Title and category are required"));
    }
    let mut extra_photos: Vec<Vec<String>> = Vec::new();
    for v in &body.variants {
        if let Err(message) = super::check_compare_at(v.price, v.compare_at_price) {
            return Ok(Response::bad_request(message));
        }
        match super::check_extra_photos(&v.image_id, &v.extra_image_ids) {
            Ok(ids) => extra_photos.push(ids),
            Err(message) => return Ok(Response::bad_request(&message)),
        }
    }

    let options = match super::check_options(&body.options) {
        Ok(o) => o,
        Err(message) => return Ok(Response::bad_request(&message)),
    };
    let mut variant_options: Vec<Vec<VariantOption>> = Vec::new();
    for v in &body.variants {
        match super::pair_values(&options, &v.options) {
            Ok(pairs) => variant_options.push(pairs),
            Err(message) => return Ok(Response::bad_request(&message)),
        }
    }
    // With no options there is only one form to sell; with options, two
    // SKUs holding the same values couldn't be told apart on the page.
    if options.is_empty() && body.variants.len() > 1 {
        return Ok(Response::bad_request("Add an option, like \"Shade\" or \"Volume\", to sell this product in more than one form."));
    }
    if super::has_duplicate_values(&variant_options) {
        return Ok(Response::bad_request("Two SKUs have the same option values."));
    }

    let slug = body.slug.clone().unwrap_or_else(|| slugify(&body.title));
    let col = db.collection::<Product>("product");
    /*
      `deleted_at: null` matters. Removing a product is a soft delete —
      remove.rs sets `deleted_at` and archives the row rather than
      dropping it — so without this clause a deleted product went on
      reserving its slug for good, and the title could never be used
      again. The unique index is scoped the same way (indexes.rs).
    */
    if col.count_documents(doc! { "slug": &slug, "deleted_at": null }).await.unwrap_or(0) > 0 {
        return Ok(Response::conflict("A product with this slug already exists"));
    }

    let now = Utc::now().timestamp_millis();
    let product_id = Uuid::now_v7().to_string();

    let product = Product {
        uuid: product_id.clone(),
        title: body.title.clone(),
        slug,
        description: body.description.clone(),
        brand_id: body.brand_id.clone(),
        category_id: body.category_id.clone(),
        tags: body.tags.clone(),
        status: body.status.clone(),
        options: options.clone(),
        // The variants are written below in the order they arrived, so the
        // first one is the oldest — the same variant recompute_aggregates will
        // pick once anything edits them.
        cover_image_id: body.variants.first().map(|v| v.image_id.clone()),
        price_min: body.variants.iter().map(|v| v.price).min().unwrap_or(0),
        price_max: body.variants.iter().map(|v| v.price).max().unwrap_or(0),
        in_stock: body.variants.iter().any(|v| v.stock > 0),
        compare_at_price: super::product_compare_at(
            &body.variants.iter().map(|v| (v.price, v.compare_at_price)).collect::<Vec<_>>(),
        ),
        rating_average: 0.0,
        rating_count: 0,
        sold_count: 0,
        created_at: now,
        created_by: admin.user_id,
        modified_at: now,
        deleted_at: None,
    };

    if let Err(error) = col.insert_one(&product).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    crate::Handler::Admin::Category::recompute_counts(&db).await;

    if !body.variants.is_empty() {
        let variants: Vec<ProductVariant> = body
            .variants
            .iter()
            .zip(variant_options)
            .zip(extra_photos)
            .map(|((v, pairs), extra_image_ids)| ProductVariant {
                uuid: Uuid::now_v7().to_string(),
                product_id: product_id.clone(),
                sku: v.sku.clone(),
                options: pairs,
                color_hex: v.color_hex.clone(),
                image_id: v.image_id.clone(),
                extra_image_ids,
                price: v.price,
                compare_at_price: v.compare_at_price,
                stock: v.stock,
                low_stock_threshold: v.low_stock_threshold,
                created_at: now,
                modified_at: now,
            })
            .collect();

        if let Err(error) = db.collection::<ProductVariant>("product_variant").insert_many(&variants).await {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(product))
}
