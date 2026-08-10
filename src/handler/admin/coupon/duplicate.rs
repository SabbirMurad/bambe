use chrono::Utc;
use uuid::Uuid;
use rand::Rng;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::utils::response::Response;
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::model::Account::AccountRole;
use crate::model::Coupon::Coupon;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let col = db.collection::<Coupon>("coupon");

    let original = col.find_one(doc! { "uuid": path.into_inner() }).await.ok().flatten();
    let original = match original {
        Some(c) => c,
        None => return Ok(Response::not_found("Coupon not found")),
    };

    let mut code = format!("{}-COPY", original.code);
    while col.count_documents(doc! { "code": &code }).await.unwrap_or(0) > 0 {
        code = format!("{}-{}", code, rand::rng().random_range(10..99));
    }

    let now = Utc::now().timestamp_millis();
    let copy = Coupon {
        uuid: Uuid::now_v7().to_string(),
        code,
        used_count: 0,
        // A duplicate starts off rather than silently going live under a
        // code nobody's seen the admin actually turn on.
        enabled: false,
        created_at: now,
        created_by: admin.user_id,
        modified_at: now,
        ..original
    };

    if let Err(error) = col.insert_one(&copy).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(copy))
}
