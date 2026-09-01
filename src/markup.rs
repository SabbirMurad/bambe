use tera::{Tera, Context};
use actix_web::{web, error, Error, HttpResponse};

/*
  Every page in this project is a static Tera template under `pages/`.
  They take no context yet, so the whole job of a handler is to name its
  template — the rendering itself is identical for all of them.
*/
fn render(template: &web::Data<Tera>, name: &str) -> Result<HttpResponse, Error> {
  let res_data = template.render(name, &Context::new())
    .map_err(|e| error::ErrorInternalServerError(e))?;

  Ok(HttpResponse::Ok().content_type("text/html").body(res_data))
}

/* ── Storefront ──────────────────────────────────────────── */

pub async fn home(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "shopping/home.html")
}

pub async fn listing(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "shopping/listing.html")
}

pub async fn product(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "shopping/product.html")
}

pub async fn search(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "shopping/search.html")
}

pub async fn wishlist(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "shopping/wishlist.html")
}

/* ── Checkout ────────────────────────────────────────────── */

pub async fn cart(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "checkout/cart.html")
}

pub async fn checkout_address(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "checkout/address.html")
}

pub async fn checkout_payment(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "checkout/payment.html")
}

pub async fn checkout_review(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "checkout/review.html")
}

pub async fn checkout_confirmation(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "checkout/confirmation.html")
}

/* ── Account ─────────────────────────────────────────────── */

pub async fn account_profile(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "account/profile.html")
}

pub async fn account_orders(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "account/orders.html")
}

pub async fn account_tracking(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "account/tracking.html")
}

pub async fn account_notifications(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "account/notifications.html")
}

pub async fn account_returns(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "account/returns.html")
}

pub async fn account_support(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "account/support.html")
}

/* ── Auth ────────────────────────────────────────────────── */

pub async fn sign_in(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/login.html")
}

pub async fn sign_up(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/register.html")
}

pub async fn forgot_password(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/forgot-password.html")
}

/* ── Admin console ───────────────────────────────────────── */

/*
  The same login markup, served under /admin so the page knows to post to
  /auth/admin/sign-in instead — admins live in `admin_account`, a different
  collection with a different route, and auth.js switches on the path.
*/
pub async fn admin_sign_in(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "auth/login.html")
}

pub async fn admin_overview(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/overview.html")
}

pub async fn admin_products(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/products.html")
}

pub async fn admin_product_new(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/product-new.html")
}

pub async fn admin_categories(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/categories.html")
}

pub async fn admin_orders(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/orders.html")
}

pub async fn admin_returns(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/returns.html")
}

pub async fn admin_customers(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/customers.html")
}

pub async fn admin_coupons(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/coupons.html")
}

pub async fn admin_support(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/support.html")
}

pub async fn admin_settings(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render(&template, "admin/settings.html")
}
