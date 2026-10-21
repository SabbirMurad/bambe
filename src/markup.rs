use tera::{Tera, Context};
use futures::StreamExt;
use mongodb::bson::doc;
use serde_json::{json, Value};
use actix_web::{web, error, Error, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::model::Category::Category;

/*
  Every page in this project is a Tera template under `pages/`. Most take
  no context, so the whole job of their handler is to name the template.
*/
async fn render(template: &web::Data<Tera>, name: &str) -> Result<HttpResponse, Error> {
  render_with(template, name, Context::new()).await
}

/*
  Every page is stamped with the store's colour theme on its <html> element
  (data-store-theme), so the right palette applies on the first frame — no
  flash of the default colours before a script could switch them.
*/
async fn store_theme() -> String {
  let db = MongoDB.connect();
  crate::Handler::Admin::Settings::load(&db).await
    .and_then(|settings| settings.theme)
    .filter(|theme| crate::Handler::Admin::Settings::UpdateStore::THEMES.contains(&theme.as_str()))
    .unwrap_or_else(|| "orange".to_string())
}

async fn render_with(template: &web::Data<Tera>, name: &str, mut context: Context) -> Result<HttpResponse, Error> {
  context.insert("store_theme", &store_theme().await);
  let res_data = template.render(name, &context)
    .map_err(|e| error::ErrorInternalServerError(e))?;

  Ok(HttpResponse::Ok().content_type("text/html").body(res_data))
}

/*
  The shop's categories, for the category bar under the navbar (home and
  listing) and the footer's Shop column (every page with a footer). Written into the
  page on the server, from the same rule the storefront's category API
  uses — visible, top level, in `position` order — so it matches the
  catalogue on the first frame, and a shop with no categories shows none
  rather than a set baked into the HTML. A failed lookup renders the bar
  without categories instead of failing the page.
*/
async fn nav_categories() -> Vec<Value> {
  let db = MongoDB.connect();
  let result = db
    .collection::<Category>("category")
    .find(doc! { "visible": true, "parent_id": null })
    .sort(doc! { "position": 1 })
    .await;

  let mut cursor = match result {
    Ok(cursor) => cursor,
    Err(error) => {
      log::error!("{}: category lookup failed: {:?}", module_path!(), error);
      return Vec::new();
    }
  };

  let mut categories = Vec::new();
  while let Some(result) = cursor.next().await {
    match result {
      Ok(c) => categories.push(json!({ "name": c.name, "slug": c.slug })),
      Err(error) => log::error!("{}: skipping unreadable category: {:?}", module_path!(), error),
    }
  }
  categories
}

async fn render_with_nav(template: &web::Data<Tera>, name: &str) -> Result<HttpResponse, Error> {
  let mut context = Context::new();
  context.insert("nav_categories", &nav_categories().await);
  render_with(template, name, context).await
}

/* ── Storefront ──────────────────────────────────────────── */

pub async fn home(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/home.html").await
}

pub async fn listing(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/listing.html").await
}

pub async fn product(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/product.html").await
}

pub async fn search(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/search.html").await
}

pub async fn wishlist(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/wishlist.html").await
}

pub async fn about(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/about.html").await
}

pub async fn privacy(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/privacy.html").await
}

pub async fn terms(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/terms.html").await
}

/* ── Checkout ────────────────────────────────────────────── */

pub async fn cart(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "checkout/cart.html").await
}

pub async fn checkout_address(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "checkout/address.html").await
}

pub async fn checkout_payment(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "checkout/payment.html").await
}

pub async fn checkout_review(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "checkout/review.html").await
}

pub async fn checkout_confirmation(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "checkout/confirmation.html").await
}

/* ── Account ─────────────────────────────────────────────── */

pub async fn account_profile(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/profile.html").await
}

pub async fn account_orders(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/orders.html").await
}

pub async fn account_tracking(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/tracking.html").await
}

pub async fn account_notifications(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/notifications.html").await
}

pub async fn account_rewards(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/rewards.html").await
}

pub async fn account_returns(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/returns.html").await
}

pub async fn account_support(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "account/support.html").await
}

/* ── Auth ────────────────────────────────────────────────── */

pub async fn sign_in(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/login.html").await
}

pub async fn sign_up(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/register.html").await
}

pub async fn forgot_password(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/forgot-password.html").await
}

/* ── Admin console ───────────────────────────────────────── */

/*
  The same login markup, served under /admin so the page knows to post to
  /auth/admin/sign-in instead — admins live in `admin_account`, a different
  collection with a different route, and auth.js switches on the path.
*/
pub async fn admin_sign_in(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/login.html").await
}

pub async fn admin_overview(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/overview.html").await
}

pub async fn admin_products(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/products.html").await
}

pub async fn admin_product_new(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/product-new.html").await
}

pub async fn admin_categories(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/categories.html").await
}

pub async fn admin_orders(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/orders.html").await
}

pub async fn admin_returns(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/returns.html").await
}

pub async fn admin_reviews(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/reviews.html").await
}

pub async fn admin_customers(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/customers.html").await
}

pub async fn admin_coupons(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/coupons.html").await
}

pub async fn admin_support(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/support.html").await
}

pub async fn admin_settings(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/settings.html").await
}
