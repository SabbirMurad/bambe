use futures::StreamExt;
use serde_json::{json, Value};
use mongodb::bson::{doc, Document};
use serde::Deserialize;
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use actix_web::{web, Error, HttpResponse, HttpRequest};
use crate::Middleware::Auth::{require_access, AccessRequirement};
use crate::Model::Account::{AccountRole, AccountDeletionRequest};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    page: Option<u64>,
    limit: Option<u64>,
}

/// Paginated queue of pending account-deletion requests, newest first, joined
/// with the user's display name.
pub async fn list(req: HttpRequest, query: web::Query<ListQuery>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let skip = (page - 1) * limit;

    let coll = db.collection::<AccountDeletionRequest>("account_deletion_request");
    let total = coll.count_documents(doc!{}).await.unwrap_or(0);

    let cursor = coll
        .find(doc!{})
        .sort(doc!{ "requested_at": 1 }) // oldest first — closest to the 15-day deadline
        .skip(skip)
        .limit(limit as i64)
        .await;

    let mut out: Vec<Value> = Vec::new();
    if let Ok(mut cursor) = cursor {
        while let Some(result) = cursor.next().await {
            let r = match result {
                Ok(r) => r,
                Err(error) => {
                    log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                    continue;
                }
            };
            let who = super::display_name(&db, &r.user_id).await;
            out.push(json!({
                "user_id": r.user_id,
                "name": who.get("name").cloned().unwrap_or(Value::Null),
                "email": r.email,
                "reason": r.reason,
                "details": r.details,
                "requested_at": r.requested_at,
            }));
        }
    }

    Ok(HttpResponse::Ok().content_type("application/json").json(json!({
        "total": total,
        "page": page,
        "limit": limit,
        "requests": out,
    })))
}

/// Permanently delete the account behind a deletion request, plus every
/// collection that keys on the user, then clear the request itself.
pub async fn delete(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let user_id = path.into_inner();

    purge_user_data(&db, &user_id).await;

    Ok(Response::ok_message("Account permanently deleted"))
}

/// Dismiss (cancel) a deletion request without deleting the account — e.g. the
/// user changed their mind or it was raised in error.
pub async fn dismiss(req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
    require_access(&req, AccessRequirement::Role(AccountRole::Administrator))?;
    let db = MongoDB.connect();
    let user_id = path.into_inner();

    let result = db
        .collection::<Document>("account_deletion_request")
        .delete_one(doc! { "user_id": &user_id })
        .await;

    match result {
        Ok(res) if res.deleted_count == 0 => Ok(Response::not_found("Request not found")),
        Ok(_) => Ok(Response::ok_message("Deletion request dismissed")),
        Err(error) => {
            log::error!("{:?}", error);
            Ok(Response::internal_server_error(&error.to_string()))
        }
    }
}

/// Hard-delete a user and all of their personal data across every collection
/// that references them. Best-effort: a failure on one collection is logged but
/// does not stop the rest, so a hiccup can't leave a half-deleted account that
/// can still sign in. Shared by the deletion-request queue and admin user
/// delete.
pub async fn purge_user_data(db: &mongodb::Database, user_id: &str) {
    // Collections keyed by `uuid`.
    for coll in ["account_core", "account_profile", "account_status"] {
        purge(db, coll, doc! { "uuid": user_id }).await;
    }

    // Collections keyed by `user_id`.
    let by_user_id = [
        "user_profile",
        "account_settings",
        "account_verification_request",
        "sign_in_verification_request",
        "discover_filters",
        "privacy_settings",
        "notification_preferences",
        "notification",
        "identity_verification",
        "fcm_token",
        "subscription_record",
        "transaction_record",
        "travel_destination",
        "conversation_participant",
        "conversation_block",
        "conversation_favorite",
        "conversation_muted",
        "account_deletion_request",
    ];
    for coll in by_user_id {
        purge(db, coll, doc! { "user_id": user_id }).await;
    }

    // Collections that reference the user under more than one field.
    purge(db, "like_record", doc! {
        "$or": [ { "liked_user_id": user_id }, { "liked_by_user_id": user_id } ]
    }).await;
    purge(db, "match_record", doc! {
        "$or": [ { "user_id_1": user_id }, { "user_id_2": user_id } ]
    }).await;
    purge(db, "pass_record", doc! {
        "$or": [ { "user_id": user_id }, { "passed_user_id": user_id } ]
    }).await;
    purge(db, "user_report", doc! { "reporter_user_id": user_id }).await;
    purge(db, "message_core", doc! { "owner": user_id }).await;
}

async fn purge(db: &mongodb::Database, collection: &str, filter: Document) {
    if let Err(error) = db
        .collection::<Document>(collection)
        .delete_many(filter)
        .await
    {
        log::error!("account deletion cleanup ({}) failed: {:?}", collection, error);
    }
}
