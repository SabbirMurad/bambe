use chrono::Utc;
use uuid::Uuid;
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use actix_web::{web, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::util::response::Response;
use crate::util::validation::validate_email;

#[derive(Debug, Deserialize)]
pub struct ReqBody {
    email_address: String,
    /// Where on the site they signed up, so the list can be segmented
    /// later. Free text, defaulted rather than required.
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewsletterSubscriber {
    pub uuid: String,
    pub email_address: String,
    pub source: Option<String>,
    pub subscribed_at: i64,
    /// Set when they unsubscribe rather than deleting the row, so a
    /// later re-subscribe does not look like a first-time signup and
    /// an unsubscribe is auditable.
    pub unsubscribed_at: Option<i64>,
}

/*
  Public newsletter signup. Deliberately unauthenticated — the footer
  form is for people who have not signed up for an account, which is
  the whole point of collecting the address.

  Because it is public and unthrottled, it answers the same way whether
  the address is new or already on the list. Saying "you are already
  subscribed" would turn the endpoint into a way of testing whether a
  given person shops here.
*/
pub async fn task(body: web::Json<ReqBody>) -> Result<HttpResponse, Error> {
    let email = body.email_address.trim().to_lowercase();

    if email.is_empty() {
        return Ok(Response::bad_request("Email is required"));
    }
    if let Err(error) = validate_email(&email) {
        return Ok(Response::bad_request(&error));
    }

    let db = MongoDB.connect();
    let collection = db.collection::<NewsletterSubscriber>("newsletter_subscriber");
    let now = Utc::now().timestamp_millis();

    match collection.find_one(doc! { "email_address": &email }).await {
        Ok(Some(existing)) => {
            // Already on the list. If they had unsubscribed, this puts
            // them back on it; otherwise there is nothing to do.
            if existing.unsubscribed_at.is_some() {
                let update = collection
                    .update_one(
                        doc! { "uuid": &existing.uuid },
                        doc! { "$set": { "unsubscribed_at": null, "subscribed_at": now } },
                    )
                    .await;
                if let Err(error) = update {
                    log::error!("{:?}", error);
                    return Ok(Response::internal_server_error(&error.to_string()));
                }
            }
        }
        Ok(None) => {
            let subscriber = NewsletterSubscriber {
                uuid: Uuid::now_v7().to_string(),
                email_address: email,
                source: body.source.clone(),
                subscribed_at: now,
                unsubscribed_at: None,
            };
            if let Err(error) = collection.insert_one(subscriber).await {
                log::error!("{:?}", error);
                return Ok(Response::internal_server_error(&error.to_string()));
            }
        }
        Err(error) => {
            log::error!("{:?}", error);
            return Ok(Response::internal_server_error(&error.to_string()));
        }
    }

    Ok(Response::ok_message("Thanks — you're on the list"))
}
