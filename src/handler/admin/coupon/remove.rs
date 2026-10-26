use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Coupon::Coupon;

pub async fn task(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Coupons).await?;
    let db = MongoDB.connect();
    let col = db.collection::<Coupon>("coupon");
    let coupon_id = path.into_inner();

    let coupon = col.find_one(doc! { "uuid": &coupon_id }).await.ok().flatten();
    let coupon = match coupon {
        Some(c) => c,
        None => return Ok(Response::not_found("Coupon not found")),
    };

    if coupon.used_count > 0 {
        return Ok(Response::bad_request("This coupon has redemptions on record — disable it instead of deleting"));
    }

    if let Err(error) = col.delete_one(doc! { "uuid": &coupon_id }).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    let name = crate::Handler::Admin::Settings::AuditLog::admin_name(&db, &admin.user_id).await;
    crate::Handler::Admin::Settings::AuditLog::log(&db, &admin.user_id, &name, "coupon.deleted", "coupon", &coupon_id, Some(coupon.code)).await;

    Ok(Response::ok_message("Coupon deleted"))
}
