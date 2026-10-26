use futures::StreamExt;
use serde_json::json;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Product::{Product, ProductVariant, Brand};
use crate::model::Category::Category;

/*
  One product and its variants, for the console's edit form.

  The public `GET /v1/product/{slug}` cannot stand in for this: it
  filters on `status: "Active"`, so a Draft — the exact thing an
  operator is most likely to be editing — comes back 404. It also keys
  on the slug, while every admin surface holds the uuid.

  Archived products are readable here on purpose: the products table
  can list them, and a row you can see but not open is a dead end.
*/
pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_staff(&req, Permission::View).await?;
    let db = MongoDB.connect();
    let product_id = path.into_inner();

    let product = db
        .collection::<Product>("product")
        .find_one(doc! { "uuid": &product_id })
        .await
        .ok()
        .flatten();
    let product = match product {
        Some(p) => p,
        None => return Ok(Response::not_found("Product not found")),
    };

    let result = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "product_id": &product_id })
        .sort(doc! { "created_at": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut variants = Vec::new();
    while let Some(next) = cursor.next().await {
        match next {
            Ok(v) => variants.push(v),
            Err(error) => log::error!("admin product detail: undeserialisable variant skipped: {:?}", error),
        }
    }

    let category = db
        .collection::<Category>("category")
        .find_one(doc! { "uuid": &product.category_id })
        .await
        .ok()
        .flatten()
        .map(|c| json!({ "uuid": c.uuid, "name": c.name, "slug": c.slug }));

    let brand = match &product.brand_id {
        Some(id) => db
            .collection::<Brand>("brand")
            .find_one(doc! { "uuid": id })
            .await
            .ok()
            .flatten()
            .map(|b| json!({ "uuid": b.uuid, "name": b.name, "slug": b.slug })),
        None => None,
    };

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "product": product,
        "variants": variants,
        "category": category,
        "brand": brand,
    })))
}
