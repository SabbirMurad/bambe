use mongodb::bson::doc;
use actix_session::Session;
use serde::{ Serialize, Deserialize };
use crate::utils::response::Response;
use actix_web::{Error, HttpResponse};

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ReqBody { fcm_token: String }


pub async fn task(actix_session: Session) -> Result<HttpResponse, Error> {

    actix_session.purge();
    
    Ok(HttpResponse::Ok().content_type("application/json").json(
        Response { message: "Successfully Signed Out".to_string() }
    ))
}