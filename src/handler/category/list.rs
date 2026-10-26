use futures::StreamExt;
use serde_json::{json, Value};
use mongodb::bson::doc;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use actix_web::{Error, HttpResponse};
use crate::model::Category::Category;

// Builds the storefront nav tree: visible categories only, nested under their
// parent, siblings ordered by `position`.
fn build_tree(parent_id: Option<&str>, categories: &[Category]) -> Vec<Value> {
    categories
        .iter()
        .filter(|c| c.parent_id.as_deref() == parent_id)
        .map(|c| {
            json!({
                "uuid": c.uuid,
                "name": c.name,
                "slug": c.slug,
                "description": c.description,
                "image_id": c.image_id,
                "product_count": c.product_count,
                "children": build_tree(Some(c.uuid.as_str()), categories),
            })
        })
        .collect()
}

pub async fn task() -> Result<HttpResponse, Error> {
    let db = MongoDB.connect();

    let result = db
        .collection::<Category>("category")
        .find(doc! { "visible": true })
        .sort(doc! { "position": 1 })
        .await;

    if let Err(error) = result {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let mut cursor = result.unwrap();
    let mut categories = Vec::new();
    while let Some(result) = cursor.next().await {
        let c = match result {
            Ok(c) => c,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        categories.push(c);
    }

    let tree = build_tree(None, &categories);

    Ok(HttpResponse::Ok().content_type("application/json").json(tree))
}
