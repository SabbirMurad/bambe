use futures::StreamExt;
use mongodb::bson::{doc, Document};
use mongodb::Database;
use crate::model::Cents;
use crate::model::Product::{Product, ProductOption, ProductVariant, VariantOption};

pub mod list;
pub use list as List;

pub mod detail;
pub use detail as Detail;

pub mod create;
pub use create as Create;

pub mod update;
pub use update as Update;

pub mod remove;
pub use remove as Remove;

pub mod add_variant;
pub use add_variant as AddVariant;

pub mod update_variant;
pub use update_variant as UpdateVariant;

pub mod remove_variant;
pub use remove_variant as RemoveVariant;

pub mod adjust_stock;
pub use adjust_stock as AdjustStock;

/*
  A variant is on sale when its "was" price is above its price. A was price
  at or below the price is a typo, not a sale, and would draw a 0% or
  negative discount, so it is refused rather than stored.
*/
pub fn check_compare_at(price: Cents, compare_at: Option<Cents>) -> Result<(), &'static str> {
    match compare_at {
        Some(was) if was <= price => Err("The \"was\" price must be higher than the price."),
        _ => Ok(()),
    }
}

/*
  The product's own `compare_at_price`, from its variants' (price, was)
  pairs. The card's discount badge and the Sale filter both measure it
  against `price_min`, so it is the was price of the cheapest variant, and
  only when that variant is itself on sale. Taking any variant's would put
  "40% off" on a card whose starting price was never discounted.
*/
pub fn product_compare_at(variants: &[(Cents, Option<Cents>)]) -> Option<Cents> {
    let cheapest = variants.iter().map(|(price, _)| *price).min()?;
    variants
        .iter()
        .filter(|(price, _)| *price == cheapest)
        .filter_map(|(price, was)| was.filter(|was| was > price))
        .max()
}

/// Photos per variant beyond its main one — ten in all.
pub const MAX_EXTRA_PHOTOS: usize = 9;

/// The extra photo ids as sent, minus blanks and repeats (including the
/// main photo), refused past MAX_EXTRA_PHOTOS.
pub fn check_extra_photos(main: &str, extra: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for id in extra {
        let id = id.trim();
        if id.is_empty() || id == main || out.iter().any(|seen| seen == id) {
            continue;
        }
        out.push(id.to_string());
    }
    if out.len() > MAX_EXTRA_PHOTOS {
        return Err(format!("A SKU can have at most {} photos.", MAX_EXTRA_PHOTOS + 1));
    }
    Ok(out)
}

/// Options per product: enough for shade x size x finish, few enough that
/// the SKU table and the product page's pickers stay readable.
pub const MAX_OPTIONS: usize = 3;
const MAX_OPTION_TEXT: usize = 60;

/*
  A product's option list as the admin sent it, tidied: names trimmed, and
  refused when one is blank, repeated (ignoring case) or too long, when
  there are more than MAX_OPTIONS, or when more than one is a swatch row.
*/
pub fn check_options(options: &[ProductOption]) -> Result<Vec<ProductOption>, String> {
    if options.len() > MAX_OPTIONS {
        return Err(format!("A product can have at most {} options.", MAX_OPTIONS));
    }
    let mut out: Vec<ProductOption> = Vec::new();
    for o in options {
        let name = o.name.trim();
        if name.is_empty() {
            return Err("Every option needs a name, like \"Shade\" or \"Volume\".".into());
        }
        if name.chars().count() > MAX_OPTION_TEXT {
            return Err(format!("\"{}\" is too long for an option name.", name));
        }
        if out.iter().any(|seen| seen.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("There are two options called \"{}\".", name));
        }
        out.push(ProductOption { name: name.to_string(), swatch: o.swatch });
    }
    if out.iter().filter(|o| o.swatch).count() > 1 {
        return Err("Only one option can be shown as colour swatches.".into());
    }
    Ok(out)
}

/*
  A variant's values, in the product's option order, paired with the option
  names. Every option needs a value: a variant with no shade on a product
  sold by shade could not be picked on the product page.
*/
pub fn pair_values(options: &[ProductOption], values: &[String]) -> Result<Vec<VariantOption>, String> {
    if values.len() != options.len() {
        return Err("Each SKU needs one value for every option.".into());
    }
    options
        .iter()
        .zip(values)
        .map(|(option, value)| {
            let value = value.trim();
            if value.is_empty() {
                Err(format!("Every SKU needs a {}.", option.name))
            } else if value.chars().count() > MAX_OPTION_TEXT {
                Err(format!("\"{}\" is too long for a {}.", value, option.name))
            } else {
                Ok(VariantOption { name: option.name.clone(), value: value.to_string() })
            }
        })
        .collect()
}

/// Whether two variants hold the same values, ignoring case.
pub fn has_duplicate_values(variants: &[Vec<VariantOption>]) -> bool {
    let mut seen = std::collections::HashSet::new();
    variants
        .iter()
        .filter(|pairs| !pairs.is_empty())
        .any(|pairs| !seen.insert(pairs.iter().map(|p| p.value.to_lowercase()).collect::<Vec<_>>()))
}

/*
  One-time move from the fixed size/colour fields to named options. Runs at
  start-up and touches only products that have no `options` field yet, so
  on a converted database it is a single empty query.

  A product whose variants had a colour gets "Colour" (shown as swatches,
  which is what colour_hex was for); one with sizes gets "Size". Each
  variant's old values become its option values, and the old fields go.
*/
pub async fn migrate_size_colour(db: &Database) {
    let products = db.collection::<Document>("product");
    let variants = db.collection::<Document>("product_variant");
    let mut cursor = match products.find(doc! { "options": { "$exists": false } }).await {
        Ok(c) => c,
        Err(error) => { log::warn!("variant option migration: {:?}", error); return; }
    };

    let text = |d: &Document, key: &str| d.get_str(key).ok().map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let mut converted = 0;
    while let Some(Ok(product)) = cursor.next().await {
        let Ok(product_id) = product.get_str("uuid").map(str::to_string) else { continue };
        let mut rows = Vec::new();
        if let Ok(mut vc) = variants.find(doc! { "product_id": &product_id }).await {
            while let Some(Ok(v)) = vc.next().await { rows.push(v); }
        }

        let has_colour = rows.iter().any(|v| text(v, "color").is_some());
        let has_size = rows.iter().any(|v| text(v, "size").is_some());
        let mut options = Vec::new();
        if has_colour { options.push(doc! { "name": "Colour", "swatch": true }); }
        if has_size { options.push(doc! { "name": "Size", "swatch": false }); }

        for v in &rows {
            let mut pairs = Vec::new();
            if has_colour { pairs.push(doc! { "name": "Colour", "value": text(v, "color").unwrap_or_else(|| "—".into()) }); }
            if has_size { pairs.push(doc! { "name": "Size", "value": text(v, "size").unwrap_or_else(|| "—".into()) }); }
            if let Ok(uuid) = v.get_str("uuid") {
                variants
                    .update_one(
                        doc! { "uuid": uuid },
                        doc! { "$set": { "options": pairs }, "$unset": { "size": "", "color": "" } },
                    )
                    .await
                    .ok();
            }
        }
        products.update_one(doc! { "uuid": &product_id }, doc! { "$set": { "options": options } }).await.ok();
        converted += 1;
    }
    if converted > 0 {
        log::info!("variant option migration: converted {} products from size/colour", converted);
    }
}

/*
  After a product's options change, rename each variant's values to match,
  position by position, so nothing is left labelled with an old name. A
  new option starts empty on every variant; the edit form saves each
  variant's real value straight after.
*/
pub async fn realign_variant_options(db: &Database, product_id: &str, options: &[ProductOption]) {
    let col = db.collection::<ProductVariant>("product_variant");
    let mut cursor = match col.find(doc! { "product_id": product_id }).await {
        Ok(c) => c,
        Err(error) => { log::error!("realign variant options: {:?}", error); return; }
    };
    let mut rows = Vec::new();
    while let Some(Ok(v)) = cursor.next().await { rows.push(v); }

    for v in rows {
        let pairs: Vec<Document> = options
            .iter()
            .enumerate()
            .map(|(i, o)| doc! {
                "name": &o.name,
                "value": v.options.get(i).map(|p| p.value.clone()).unwrap_or_default(),
            })
            .collect();
        col.update_one(doc! { "uuid": &v.uuid }, doc! { "$set": { "options": pairs } }).await.ok();
    }
}

/*
  price_min/price_max/in_stock/cover_image_id on `product` are denormalised
  from its variants for the listing page's price and stock filters and its
  card thumbnail, and compare_at_price for its sale badge and Sale filter.
  Call this after any variant write (create, edit, delete, restock) so the
  product row never drifts from what its variants actually say.
*/
pub async fn recompute_aggregates(db: &Database, product_id: &str) {
    let mut cursor = db
        .collection::<ProductVariant>("product_variant")
        .find(doc! { "product_id": product_id })
        .await
        .expect("variant lookup");
    let mut variants = Vec::new();
    while let Some(result) = cursor.next().await {
        let v = match result {
            Ok(v) => v,
            Err(error) => {
                log::error!("{}: skipping unreadable document: {:?}", module_path!(), error);
                continue;
            }
        };
        variants.push(v);
    }

    let price_min = variants.iter().map(|v| v.price).min().unwrap_or(0);
    let price_max = variants.iter().map(|v| v.price).max().unwrap_or(0);
    let in_stock = variants.iter().any(|v| v.stock > 0);
    let compare_at_price = product_compare_at(
        &variants.iter().map(|v| (v.price, v.compare_at_price)).collect::<Vec<_>>(),
    );
    // The oldest variant is the cover, so the thumbnail stays put as later
    // colourways are added — sorted here rather than trusted from the cursor,
    // which has no order of its own.
    let cover_image_id = variants
        .iter()
        .min_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.uuid.cmp(&b.uuid)))
        .map(|v| v.image_id.clone());

    db.collection::<Product>("product")
        .update_one(
            doc! { "uuid": product_id },
            doc! {
                "$set": {
                    "price_min": price_min,
                    "price_max": price_max,
                    "in_stock": in_stock,
                    "cover_image_id": cover_image_id,
                    "compare_at_price": compare_at_price,
                },
            },
        )
        .await
        .ok();
}
