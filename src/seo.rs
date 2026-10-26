//! What search engines read: per-page titles and descriptions, canonical
//! URLs, Open Graph tags, schema.org structured data, the sitemap, robots.txt
//! and the Google Merchant product feed.
//!
//! Everything is built from the catalogue at request time. Publishing a
//! product (status Active) is all it takes for it to get its own indexable
//! page, a sitemap entry and a feed item; archiving one takes all three away.

use std::collections::HashMap;
use futures::StreamExt;
use mongodb::Database;
use mongodb::bson::doc;
use serde_json::{json, Value};
use actix_web::{Error, HttpRequest, HttpResponse};
use crate::BuiltIns::mongo::MongoDB;
use crate::model::Category::Category;
use crate::model::Product::{Brand, Product, ProductVariant};

pub const SITE_NAME: &str = "Bambee";
const DESCRIPTION_MAX: usize = 158;

/// The public origin, e.g. "https://bambee.store". SITE_URL when set (it
/// must be in production: canonical URLs should never depend on which
/// hostname a crawler happened to use), otherwise whatever this request
/// came in on.
pub fn base_url(req: &HttpRequest) -> String {
    match std::env::var("SITE_URL") {
        Ok(url) if !url.trim().is_empty() => url.trim().trim_end_matches('/').to_string(),
        _ => {
            let info = req.connection_info();
            format!("{}://{}", info.scheme(), info.host())
        }
    }
}

/// Percent-encodes a path segment or query value (RFC 3986 unreserved
/// characters pass through). Slugs are usually already URL-safe; this
/// covers the ones that aren't.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn product_path(slug: &str) -> String {
    format!("/product/{}", encode(slug))
}

pub fn image_url(base: &str, image_id: &str) -> String {
    format!("{}/image/webp/{}", base, image_id)
}

/// Collapses whitespace and cuts at a word boundary, for meta descriptions.
pub fn plain(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max - 1).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > max / 2 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}…", cut.trim_end_matches(|c: char| c == ',' || c == '.' || c == ';' || c == ':'))
}

pub fn money(cents: i64) -> String {
    format!("{}.{:02}", cents / 100, (cents % 100).abs())
}

/// JSON for a <script type="application/ld+json"> block. `</` is escaped so
/// a product description can never close the script element early.
pub fn ld(value: &Value) -> String {
    value.to_string().replace("</", "<\\/")
}

pub async fn currency(db: &Database) -> String {
    crate::Handler::Admin::Settings::load(db)
        .await
        .map(|s| s.default_currency)
        .unwrap_or_else(|| "BDT".to_string())
}

/// The `seo` object every public template's head reads (common/seo.html).
/// The 1200x630 card link previews fall back to on pages with no picture of
/// their own — a preview with no image is easy to scroll past.
pub const DEFAULT_SHARE_IMAGE: &str = "/assets/brand/og-image.png";

pub fn page(title: &str, description: &str, canonical: String, image: Option<String>) -> Value {
    // The origin is the canonical URL up to its path.
    let origin = canonical.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
    let (image, default_image) = match image {
        Some(image) => (image, false),
        None => (format!("{}{}", origin, DEFAULT_SHARE_IMAGE), true),
    };
    json!({
        "title": title,
        "description": plain(description, DESCRIPTION_MAX),
        "canonical": canonical,
        "image": image,
        // Its size is known, so previews can lay out before fetching it.
        "image_size": if default_image { json!([1200, 630]) } else { Value::Null },
        "og_type": "website",
        "noindex": false,
        "json_ld": Vec::<String>::new(),
    })
}

/*
  Other names people use for the shop. Google reads WebSite.alternateName
  when choosing the site name it shows in results, and the Organization's
  for matching a search to the business. Deliberately no misspellings:
  Google corrects those itself, and listing them reads as stuffing.
*/
const ALTERNATE_NAMES: [&str; 2] = ["Bambee Store", "bambee.store"];

/// OnlineStore (an Organization) + WebSite (with the sitelinks search box),
/// for the home page. `areaServed` is what sets this Bambee apart from the
/// unrelated businesses abroad that share the name.
pub fn home_json_ld(base: &str) -> Vec<String> {
    vec![
        ld(&json!({
            "@context": "https://schema.org",
            "@type": "OnlineStore",
            "name": SITE_NAME,
            "alternateName": ALTERNATE_NAMES,
            "description": "Online beauty and cosmetics shop in Bangladesh.",
            "url": format!("{}/", base),
            "logo": format!("{}/assets/favicon/web-app-manifest-512x512.png", base),
            "areaServed": { "@type": "Country", "name": "Bangladesh" },
        })),
        ld(&json!({
            "@context": "https://schema.org",
            "@type": "WebSite",
            "name": SITE_NAME,
            "alternateName": ALTERNATE_NAMES,
            "url": format!("{}/", base),
            "potentialAction": {
                "@type": "SearchAction",
                "target": { "@type": "EntryPoint", "urlTemplate": format!("{}/listing?q={{search_term_string}}", base) },
                "query-input": "required name=search_term_string",
            },
        })),
    ]
}

/// Everything the product page's head and first paint need, or None when
/// the slug isn't a live product.
pub async fn product_page(db: &Database, base: &str, slug: &str) -> Option<(Value, Value)> {
    let product = db
        .collection::<Product>("product")
        .find_one(doc! { "slug": slug, "status": "Active", "deleted_at": null })
        .await
        .ok()
        .flatten()?;

    let brand = match &product.brand_id {
        Some(id) => db.collection::<Brand>("brand").find_one(doc! { "uuid": id }).await.ok().flatten(),
        None => None,
    };
    let category = db
        .collection::<Category>("category")
        .find_one(doc! { "uuid": &product.category_id })
        .await
        .ok()
        .flatten();

    let mut variants = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "product_id": &product.uuid })
        .sort(doc! { "price": 1 })
        .await
    {
        while let Some(Ok(v)) = cursor.next().await {
            variants.push(v);
        }
    }

    let currency = currency(db).await;
    let url = format!("{}{}", base, product_path(&product.slug));
    let mut images: Vec<String> = Vec::new();
    for id in product.cover_image_id.iter().chain(variants.iter().map(|v| &v.image_id)) {
        let u = image_url(base, id);
        if !images.contains(&u) { images.push(u); }
    }

    let brand_name = brand.as_ref().map(|b| b.name.clone());
    let title = match &brand_name {
        Some(b) if !product.title.to_lowercase().contains(&b.to_lowercase()) => format!("{} by {} | {}", product.title, b, SITE_NAME),
        _ => format!("{} | {}", product.title, SITE_NAME),
    };
    let description = if product.description.trim().is_empty() {
        format!(
            "Shop {}{} at {}{}. Delivered to your door.",
            product.title,
            brand_name.as_ref().map(|b| format!(" by {}", b)).unwrap_or_default(),
            SITE_NAME,
            category.as_ref().map(|c| format!(" — {}", c.name)).unwrap_or_default(),
        )
    } else {
        product.description.clone()
    };

    let availability = if product.in_stock { "https://schema.org/InStock" } else { "https://schema.org/OutOfStock" };
    let offers = if variants.len() > 1 && product.price_min != product.price_max {
        json!({
            "@type": "AggregateOffer",
            "priceCurrency": currency,
            "lowPrice": money(product.price_min),
            "highPrice": money(product.price_max),
            "offerCount": variants.len(),
            "availability": availability,
            "url": url,
        })
    } else {
        json!({
            "@type": "Offer",
            "priceCurrency": currency,
            "price": money(product.price_min),
            "availability": availability,
            "itemCondition": "https://schema.org/NewCondition",
            "url": url,
        })
    };

    let mut schema = json!({
        "@context": "https://schema.org",
        "@type": "Product",
        "name": product.title,
        "description": plain(&description, 5000),
        "image": images,
        "url": url,
        "offers": offers,
    });
    if let Some(v) = variants.first() { schema["sku"] = json!(v.sku); }
    if let Some(b) = &brand_name { schema["brand"] = json!({ "@type": "Brand", "name": b }); }
    if let Some(c) = &category { schema["category"] = json!(c.name); }
    if product.rating_count > 0 {
        schema["aggregateRating"] = json!({
            "@type": "AggregateRating",
            "ratingValue": format!("{:.1}", product.rating_average),
            "reviewCount": product.rating_count,
        });
    }

    let mut crumbs = vec![json!({ "@type": "ListItem", "position": 1, "name": "Home", "item": format!("{}/", base) })];
    if let Some(c) = &category {
        crumbs.push(json!({
            "@type": "ListItem", "position": 2, "name": c.name,
            "item": format!("{}/listing?category={}", base, encode(&c.slug)),
        }));
    }
    crumbs.push(json!({ "@type": "ListItem", "position": crumbs.len() + 1, "name": product.title, "item": url }));
    let breadcrumbs = json!({ "@context": "https://schema.org", "@type": "BreadcrumbList", "itemListElement": crumbs });

    let mut seo = page(&title, &description, url, images.first().cloned());
    seo["og_type"] = json!("product");
    seo["json_ld"] = json!([ld(&schema), ld(&breadcrumbs)]);
    seo["product_price"] = json!(money(product.price_min));
    seo["product_currency"] = json!(currency);

    // Painted into the page so crawlers and the first frame have the text;
    // product.js fills the same elements again once it loads.
    let first_paint = json!({
        "title": product.title,
        "description": product.description,
        "brand": brand_name,
    });
    Some((seo, first_paint))
}

/* ── /favicon.ico ─────────────────────────────────────────────
   Browsers request it from the root whatever the page's <link rel="icon">
   says, and Google's search-result favicon fetcher looks there too.     */

pub async fn favicon() -> actix_web::Result<actix_files::NamedFile> {
    Ok(actix_files::NamedFile::open("assets/favicon/favicon.ico")?)
}

/* ── robots.txt ─────────────────────────────────────────────── */

pub async fn robots(req: HttpRequest) -> Result<HttpResponse, Error> {
    let base = base_url(&req);
    let body = format!(
        "User-agent: *\n\
         Allow: /\n\
         Disallow: /admin\n\
         Disallow: /account\n\
         Disallow: /cart\n\
         Disallow: /checkout\n\
         Disallow: /api/\n\
         Disallow: /login\n\
         Disallow: /register\n\
         Disallow: /forgot-password\n\
         \n\
         Sitemap: {}/sitemap.xml\n",
        base
    );
    Ok(HttpResponse::Ok().content_type("text/plain; charset=utf-8").body(body))
}

/* ── sitemap.xml ────────────────────────────────────────────── */

fn xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn w3c_date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

async fn live_products(db: &Database) -> Vec<Product> {
    let mut out = Vec::new();
    if let Ok(mut cursor) = db
        .collection::<Product>("product")
        .find(doc! { "status": "Active", "deleted_at": null })
        .sort(doc! { "modified_at": -1 })
        .await
    {
        while let Some(result) = cursor.next().await {
            match result {
                Ok(p) => out.push(p),
                Err(error) => log::error!("{}: skipping unreadable product: {:?}", module_path!(), error),
            }
        }
    }
    out
}

pub async fn sitemap(req: HttpRequest) -> Result<HttpResponse, Error> {
    let base = base_url(&req);
    let db = MongoDB.connect();

    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" \
         xmlns:image=\"http://www.google.com/schemas/sitemap-image/1.1\">\n",
    );
    for path in ["/", "/listing", "/about", "/privacy", "/terms", "/return-policy"] {
        out.push_str(&format!("  <url><loc>{}</loc></url>\n", xml(&format!("{}{}", base, path))));
    }

    if let Ok(mut cursor) = db.collection::<Category>("category").find(doc! { "visible": true }).await {
        while let Some(Ok(c)) = cursor.next().await {
            out.push_str(&format!(
                "  <url><loc>{}</loc></url>\n",
                xml(&format!("{}/listing?category={}", base, encode(&c.slug)))
            ));
        }
    }

    for p in live_products(&db).await {
        out.push_str(&format!(
            "  <url><loc>{}</loc><lastmod>{}</lastmod>",
            xml(&format!("{}{}", base, product_path(&p.slug))),
            w3c_date(p.modified_at),
        ));
        if let Some(id) = &p.cover_image_id {
            out.push_str(&format!(
                "<image:image><image:loc>{}</image:loc></image:image>",
                xml(&image_url(&base, id))
            ));
        }
        out.push_str("</url>\n");
    }
    out.push_str("</urlset>\n");

    Ok(HttpResponse::Ok().content_type("application/xml; charset=utf-8").body(out))
}

/* ── Google Merchant feed ──────────────────────────────────────
   One item per variant, grouped by product, for free listings in Google
   Shopping. Add {SITE_URL}/feeds/google.xml as a scheduled feed in
   Merchant Center and it re-reads the catalogue on its own schedule.  */

pub async fn merchant_feed(req: HttpRequest) -> Result<HttpResponse, Error> {
    let base = base_url(&req);
    let db = MongoDB.connect();
    let currency = currency(&db).await;
    let products = live_products(&db).await;

    let ids: Vec<String> = products.iter().map(|p| p.uuid.clone()).collect();
    let mut variants: HashMap<String, Vec<ProductVariant>> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<ProductVariant>("product_variant").find(doc! { "product_id": { "$in": &ids } }).await {
        while let Some(Ok(v)) = cursor.next().await {
            variants.entry(v.product_id.clone()).or_default().push(v);
        }
    }
    let mut brands: HashMap<String, String> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Brand>("brand").find(doc! {}).await {
        while let Some(Ok(b)) = cursor.next().await {
            brands.insert(b.uuid, b.name);
        }
    }
    let mut categories: HashMap<String, String> = HashMap::new();
    if let Ok(mut cursor) = db.collection::<Category>("category").find(doc! {}).await {
        while let Some(Ok(c)) = cursor.next().await {
            categories.insert(c.uuid, c.name);
        }
    }

    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <rss version=\"2.0\" xmlns:g=\"http://base.google.com/ns/1.0\">\n<channel>\n\
         <title>{}</title>\n<link>{}/</link>\n<description>{} product feed</description>\n",
        SITE_NAME, xml(&base), SITE_NAME,
    );

    for p in &products {
        let link = format!("{}{}", base, product_path(&p.slug));
        let description = if p.description.trim().is_empty() { p.title.clone() } else { plain(&p.description, 5000) };
        for v in variants.get(&p.uuid).map(|v| v.as_slice()).unwrap_or(&[]) {
            let label: Vec<&str> = v.options.iter().map(|o| o.value.as_str()).collect();
            let title = if label.is_empty() { p.title.clone() } else { format!("{} - {}", p.title, label.join(" / ")) };
            /* Google knows colour and size, not the names an admin types, so
               the swatch option counts as colour and one named for size,
               volume or weight counts as size. Anything else stays in the
               title only. */
            let colour = p.options.iter().find(|o| o.swatch).and_then(|o| v.option(&o.name));
            let size = p
                .options
                .iter()
                .find(|o| {
                    let n = o.name.to_lowercase();
                    !o.swatch && (n.contains("size") || n.contains("volume") || n.contains("weight"))
                })
                .and_then(|o| v.option(&o.name));
            let on_sale = v.compare_at_price.map(|was| was > v.price).unwrap_or(false);

            out.push_str("<item>\n");
            out.push_str(&format!("  <g:id>{}</g:id>\n", xml(if v.sku.trim().is_empty() { &v.uuid } else { &v.sku })));
            out.push_str(&format!("  <g:item_group_id>{}</g:item_group_id>\n", xml(&p.uuid)));
            out.push_str(&format!("  <g:title>{}</g:title>\n", xml(&plain(&title, 150))));
            out.push_str(&format!("  <g:description>{}</g:description>\n", xml(&description)));
            out.push_str(&format!("  <g:link>{}</g:link>\n", xml(&link)));
            out.push_str(&format!("  <g:image_link>{}</g:image_link>\n", xml(&image_url(&base, &v.image_id))));
            for extra in &v.extra_image_ids {
                out.push_str(&format!("  <g:additional_image_link>{}</g:additional_image_link>\n", xml(&image_url(&base, extra))));
            }
            out.push_str(&format!("  <g:availability>{}</g:availability>\n", if v.stock > 0 { "in_stock" } else { "out_of_stock" }));
            if on_sale {
                out.push_str(&format!("  <g:price>{} {}</g:price>\n", money(v.compare_at_price.unwrap()), currency));
                out.push_str(&format!("  <g:sale_price>{} {}</g:sale_price>\n", money(v.price), currency));
            } else {
                out.push_str(&format!("  <g:price>{} {}</g:price>\n", money(v.price), currency));
            }
            out.push_str("  <g:condition>new</g:condition>\n");
            match p.brand_id.as_ref().and_then(|id| brands.get(id)) {
                Some(b) => out.push_str(&format!("  <g:brand>{}</g:brand>\n  <g:identifier_exists>no</g:identifier_exists>\n", xml(b))),
                None => out.push_str("  <g:identifier_exists>no</g:identifier_exists>\n"),
            }
            if let Some(c) = categories.get(&p.category_id) {
                out.push_str(&format!("  <g:product_type>{}</g:product_type>\n", xml(c)));
            }
            if let Some(color) = colour { out.push_str(&format!("  <g:color>{}</g:color>\n", xml(color))); }
            if let Some(size) = size { out.push_str(&format!("  <g:size>{}</g:size>\n", xml(size))); }
            out.push_str("</item>\n");
        }
    }
    out.push_str("</channel>\n</rss>\n");

    Ok(HttpResponse::Ok().content_type("application/xml; charset=utf-8").body(out))
}
