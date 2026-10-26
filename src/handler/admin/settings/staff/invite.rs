use chrono::Utc;
use uuid::Uuid;
use serde::Deserialize;
use mongodb::bson::doc;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Handler::Admin::Access::{require_staff, Permission};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::model::Account::AdminAccount;
use crate::model::Store::{StaffMember, StaffRole, StaffStatus};

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    email: String,
    role: StaffRole,
}

/*
  Credentials live in `admin_account`, created through the admin sign-up
  flow — this endpoint only ever attaches a console profile (role, status)
  to an account that already exists, rather than inventing a parallel
  invite-by-email system with its own account creation.
*/
pub async fn task(req: HttpRequest, body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let admin = require_staff(&req, Permission::Staff).await?;
    let db = MongoDB.connect();

    let account = db
        .collection::<AdminAccount>("admin_account")
        .find_one(doc! { "email_address": &body.email })
        .await
        .ok()
        .flatten();
    let account = match account {
        Some(a) => a,
        None => return Ok(Response::bad_request("No admin account found for this email — they need to sign up first")),
    };

    let staff_col = db.collection::<StaffMember>("staff_member");
    if staff_col.count_documents(doc! { "admin_id": &account.uuid }).await.unwrap_or(0) > 0 {
        return Ok(Response::conflict("This admin is already on the staff list"));
    }

    let now = Utc::now().timestamp_millis();
    let member = StaffMember {
        uuid: Uuid::now_v7().to_string(),
        admin_id: account.uuid,
        name: if account.name.is_empty() { body.email.clone() } else { account.name },
        email: body.email.clone(),
        role: body.role.clone(),
        status: StaffStatus::Active,
        avatar_image_id: None,
        last_active_at: None,
        invited_by: Some(admin.user_id),
        invited_at: now,
        joined_at: Some(now),
    };

    if let Err(error) = staff_col.insert_one(&member).await {
        log::error!("{:?}", error);
        return Ok(Response::internal_server_error(&error.to_string()));
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(member))
}
