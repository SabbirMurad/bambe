use tera::{Tera, Context};
use futures::StreamExt;
use mongodb::bson::doc;
use serde_json::{json, Value};
use actix_web::{web, error, Error, HttpRequest, HttpResponse};
use actix_web::http::StatusCode;
use std::collections::HashMap;
use crate::seo;
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
struct StoreChrome {
  theme: String,
  whatsapp: Option<String>,
  facebook: Option<String>,
  instagram: Option<String>,
}

async fn store_chrome() -> StoreChrome {
  let db = MongoDB.connect();
  let settings = crate::Handler::Admin::Settings::load(&db).await;
  let theme = settings.as_ref()
    .and_then(|settings| settings.theme.clone())
    .filter(|theme| crate::Handler::Admin::Settings::UpdateStore::THEMES.contains(&theme.as_str()))
    .unwrap_or_else(|| "orange".to_string());
  /* The footer's WhatsApp button chats with the store's support phone.
     wa.me wants the full international number as bare digits — no +,
     spaces or dashes — and opens the app where it's installed, WhatsApp
     Web or the download page where it isn't. */
  let whatsapp = settings.as_ref()
    .and_then(|settings| settings.support_phone.clone())
    .map(|phone| phone.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
    .filter(|digits| digits.len() >= 8);
  let link = |value: Option<String>| value.map(|v| v.trim().to_string()).filter(|v| v.starts_with("http"));
  StoreChrome {
    theme,
    whatsapp,
    facebook: link(settings.as_ref().and_then(|s| s.facebook_url.clone())),
    instagram: link(settings.as_ref().and_then(|s| s.instagram_url.clone())),
  }
}

async fn render_with(template: &web::Data<Tera>, name: &str, context: Context) -> Result<HttpResponse, Error> {
  render_status(template, name, context, StatusCode::OK).await
}

async fn render_status(template: &web::Data<Tera>, name: &str, mut context: Context, status: StatusCode) -> Result<HttpResponse, Error> {
  let chrome = store_chrome().await;
  context.insert("store_theme", &chrome.theme);
  if let Some(number) = &chrome.whatsapp { context.insert("whatsapp_number", number); }
  if let Some(url) = &chrome.facebook { context.insert("facebook_url", url); }
  if let Some(url) = &chrome.instagram { context.insert("instagram_url", url); }
  // The home page's Organization data lists the same profiles.
  let same_as: Vec<&String> = chrome.facebook.iter().chain(chrome.instagram.iter()).collect();
  if let Some(seo) = context.get("seo").cloned() {
    if seo.get("json_ld").is_some() && !same_as.is_empty() {
      let mut seo = seo;
      if let Some(blocks) = seo["json_ld"].as_array_mut() {
        for block in blocks.iter_mut() {
          if let Some(text) = block.as_str() {
            if let Ok(mut value) = serde_json::from_str::<Value>(text) {
              if value["@type"] == "Organization" {
                value["sameAs"] = json!(same_as);
                *block = json!(seo::ld(&value));
              }
            }
          }
        }
      }
      context.insert("seo", &seo);
    }
  }
  // Search Console's ownership check, when GOOGLE_SITE_VERIFICATION is set.
  if let Ok(code) = std::env::var("GOOGLE_SITE_VERIFICATION") {
    let code = code.trim();
    if !code.is_empty() && !code.to_ascii_uppercase().contains("CHANGE_ME") {
      context.insert("google_site_verification", code);
    }
  }
  let res_data = template.render(name, &context)
    .map_err(|e| error::ErrorInternalServerError(e))?;

  Ok(HttpResponse::build(status).content_type("text/html").body(res_data))
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
  render_with(template, name, nav_context().await).await
}

async fn nav_context() -> Context {
  let mut context = Context::new();
  context.insert("nav_categories", &nav_categories().await);
  context
}

/// A public page with its own title, description and canonical URL.
async fn render_seo(template: &web::Data<Tera>, name: &str, seo: Value) -> Result<HttpResponse, Error> {
  let mut context = nav_context().await;
  context.insert("seo", &seo);
  render_with(template, name, context).await
}

/* ── Storefront ──────────────────────────────────────────── */

pub async fn home(template: web::Data<Tera>, req: HttpRequest) -> Result<HttpResponse, Error> {
  let base = seo::base_url(&req);
  let mut page = seo::page(
    "Bambee — Cosmetics, Skincare, Body, Hair & Grooming",
    "Bambee stocks seven lines of everyday beauty: cosmetics and accessories, skincare, body care and sprays, wellness, hair care and extensions, nail care, and men's grooming.",
    format!("{}/", base),
    None,
  );
  page["json_ld"] = json!(seo::home_json_ld(&base));
  render_seo(&template, "shopping/home.html", page).await
}

/*
  One indexable page per category. Sorting, paging and filter variants of
  it point their canonical back at the plain category URL, and a search
  results page (`q`) is kept out of the index: it's endless, thin, and
  every product on it already has a page of its own.
*/
pub async fn listing(template: web::Data<Tera>, req: HttpRequest, query: web::Query<HashMap<String, String>>) -> Result<HttpResponse, Error> {
  let base = seo::base_url(&req);
  let db = MongoDB.connect();

  let category = match query.get("category").filter(|c| !c.is_empty()) {
    Some(slug) => db.collection::<Category>("category").find_one(doc! { "slug": slug, "visible": true }).await.ok().flatten(),
    None => None,
  };

  let mut page = match &category {
    Some(c) => seo::page(
      &format!("{} — Shop {} Online | {}", c.name, c.name, seo::SITE_NAME),
      &c.description.clone().filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| format!("Shop {} at {}: hand-picked products from brands you trust, delivered to your door.", c.name, seo::SITE_NAME)),
      format!("{}/listing?category={}", base, seo::encode(&c.slug)),
      c.image_id.as_ref().map(|id| seo::image_url(&base, id)),
    ),
    None => seo::page(
      &format!("Shop All Beauty Products | {}", seo::SITE_NAME),
      "Browse every product at Bambee — makeup, skincare, body care, wellness, hair care, nail care and men's grooming.",
      format!("{}/listing", base),
      None,
    ),
  };
  if query.get("q").map(|q| !q.trim().is_empty()).unwrap_or(false) {
    page["noindex"] = json!(true);
  }
  render_seo(&template, "shopping/listing.html", page).await
}

/// `/product/{slug}` — the canonical product page.
pub async fn product(template: web::Data<Tera>, req: HttpRequest, path: web::Path<String>) -> Result<HttpResponse, Error> {
  product_page(&template, &req, &path.into_inner()).await
}

async fn product_page(template: &web::Data<Tera>, req: &HttpRequest, slug: &str) -> Result<HttpResponse, Error> {
  let base = seo::base_url(req);
  let db = MongoDB.connect();

  let mut context = nav_context().await;
  match seo::product_page(&db, &base, slug).await {
    Some((page, first_paint)) => {
      context.insert("seo", &page);
      context.insert("product_ssr", &first_paint);
      render_with(template, "shopping/product.html", context).await
    }
    None => {
      // Still the product page, so product.js can say "not found" in the
      // shop's own chrome, but a real 404, so it never gets indexed.
      let mut page = seo::page(&format!("Product not found | {}", seo::SITE_NAME), "", format!("{}/listing", base), None);
      page["noindex"] = json!(true);
      context.insert("seo", &page);
      render_status(template, "shopping/product.html", context, StatusCode::NOT_FOUND).await
    }
  }
}

/// The old `/product?slug=…` address, kept working for links already out
/// there: a permanent redirect hands their ranking to the new URL.
pub async fn product_legacy(template: web::Data<Tera>, req: HttpRequest, query: web::Query<HashMap<String, String>>) -> Result<HttpResponse, Error> {
  match query.get("slug").filter(|s| !s.is_empty()) {
    Some(slug) => Ok(HttpResponse::MovedPermanently()
      .append_header(("Location", seo::product_path(slug)))
      .finish()),
    None => product_page(&template, &req, "").await,
  }
}

pub async fn search(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/search.html").await
}

pub async fn wishlist(template: web::Data<Tera>) -> Result<HttpResponse, Error> {
  render_with_nav(&template, "shopping/wishlist.html").await
}

pub async fn about(template: web::Data<Tera>, req: HttpRequest) -> Result<HttpResponse, Error> {
  let base = seo::base_url(&req);
  let page = seo::page("About Us | Bambee", "At Bambee, we believe true beauty starts with intentional self-care: clean, salon-grade skincare, hair care, cosmetics and men's grooming, curated for every skin tone, lifestyle and beauty goal.", format!("{}/about", base), None);
  render_seo(&template, "shopping/about.html", page).await
}

pub async fn privacy(template: web::Data<Tera>, req: HttpRequest) -> Result<HttpResponse, Error> {
  let base = seo::base_url(&req);
  let page = seo::page("Privacy Policy | Bambee", "How Bambee collects, uses and protects your personal information — and the choices you have.", format!("{}/privacy", base), None);
  render_seo(&template, "shopping/privacy.html", page).await
}

pub async fn return_policy(template: web::Data<Tera>, req: HttpRequest) -> Result<HttpResponse, Error> {
  let base = seo::base_url(&req);
  let page = seo::page("Return & Refund Policy | Bambee", "10-day returns with free return shipping. What you can return, how to start a return, and how refunds work at Bambee.", format!("{}/return-policy", base), None);
  render_seo(&template, "shopping/return-policy.html", page).await
}

pub async fn terms(template: web::Data<Tera>, req: HttpRequest) -> Result<HttpResponse, Error> {
  let base = seo::base_url(&req);
  let page = seo::page("Terms of Use | Bambee", "The terms that apply when you use the Bambee website and shop with us.", format!("{}/terms", base), None);
  render_seo(&template, "shopping/terms.html", page).await
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
