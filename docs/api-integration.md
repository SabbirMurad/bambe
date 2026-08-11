# API integration checklist

The backend serves the full REST surface. This is the list of what has to be
wired, page by page, and what has to exist before any of it can work.

## Progress

- [x] **Prerequisites** — `session.js` (tokens, guest cart id, refresh),
      `Authorization` and `X-Guest-Session` injection in `fetcher.js`, 401
      refresh-and-retry, and the two `fetcher.js` bugs below.
- [x] **Auth** — login, register (with verification step), forgot-password.
      `assets/js/auth.js`.
- [x] **Listing and search** — filters, facets from the API, sort, pagination,
      URL sync, result counts. `assets/js/listing.js` + `assets/js/catalog.js`.
- [x] **Home** — category strip and both product rows. `assets/js/home.js`.
- [x] **Product detail** — gallery, variant picker, price/stock per variant,
      cart, wishlist, reviews, related. `assets/js/product.js`.
- [x] **Wishlist** — rows, price-drop callout, remove, add to cart.
      `assets/js/wishlist.js`.
- [ ] Cart and checkout
- [ ] Account
- [ ] Admin console

Three more pre-existing bugs were fixed on the way through:

3. **`skeleton.js` inserted its placeholder clone *before* the real element.**
   The clone is a full copy — same classes — so while mounted it shadowed the
   original in every `querySelector`. A page module that filled in the product
   title wrote into the placeholder, and the swap back then restored the
   wireframe's dummy text over live data. It now mounts after the target, and
   exposes `window.Skeleton.clear(el)` so a page can drop the placeholder the
   moment its data lands instead of waiting out a fixed 1.4s timer.
4. **`DELETE /v1/wishlist/{id}` takes the wishlist *row* id, not the product
   id.** Both heart toggles passed the product id and would have 404'd.
   `catalog.js` now keeps a product_id → row_id index loaded from
   `GET /v1/wishlist`.
5. `productCard` read `options.wished`, which is per-grid, so it would have
   painted every card in a grid the same. It reads the index per item now.

Two `fetcher.js` bugs were fixed along the way, both pre-existing:

1. The query builder appended every non-array key **twice**, so `{rating: 5}`
   went out as `rating=5&rating=5`. actix's `web::Query` treats a repeated key
   for a non-`Vec` field as a deserialise error, so **every single-value filter
   returned 400**. It also never encoded anything, so a search term containing
   `&` or `#` truncated the query. Replaced with `Fetcher.buildQueryString`,
   built on `URLSearchParams`.
2. The `catch` around `response.json()` on a *successful* response referenced an
   `error` binding scoped to the `!response.ok` branch, throwing a
   `ReferenceError` that buried the real failure.

And one backend change, so the two sides agree: `product/list` now splits
`brand`, `size` and `color` on commas and matches any of them. They were
single-value, but the sidebar is multi-select — ticking three brands and
filtering by one is worse than not offering the tick.

Every call goes through `Fetcher` (`assets/js/fetcher.js`) and reports through
`toast` (`assets/js/toast.js`).

---

## Before anything else

Four things block every page below. They are worth doing first, in one pass, or
the same workaround gets written thirty times.

### 1. `Fetcher` sends no `Authorization` header

Every account, checkout and admin endpoint calls `require_access`, which reads
`Authorization: Bearer <token>`. `Fetcher.#commonMethods` never sets it, so
today every one of those calls returns 401.

Add to `fetcher.js`, not to each caller:

- a token store (`localStorage`), written by sign-in and cleared by sign-out;
- injection of `Authorization` into `_headers` when a token is present;
- on a 401, one attempt at `POST /api/auth/refresh`, then a single retry of the
  original request; on a second failure, clear the token and redirect to
  `/login`.

The refresh route is deliberately registered outside the `/api/v1` scope
(`src/routes/auth.rs`), so it is `/api/auth/refresh` exactly.

### 2. `Fetcher` sends no guest-session header

The cart works signed-out. `resolve_owner` (`src/handler/cart.rs`) takes the
signed-in user if there is one, and otherwise keys the cart on the
`X-Guest-Session` header — a client-generated id.

- Mint a uuid on first cart interaction, persist it in `localStorage`.
- Send it on every `/v1/cart/*` call while signed out.
- On sign-in, `POST /v1/cart/merge` to fold the guest cart into the user's, then
  drop the id.

Without this every guest cart call returns "cart is empty".

### 3. Endpoint paths are relative to `/api`

`Fetcher` builds `(window.projectDomain || '') + "/api" + endpoint`. So pass
`/v1/product/list`, never `/api/v1/product/list`.

The one exception is image upload: `POST /image` is registered outside the
`/api` scope and takes multipart, while `Fetcher` hardcodes
`Content-Type: application/json`. Image uploads need a raw `fetch` with
`FormData` and no explicit content type.

### Uploading images

`POST /image`, multipart, authenticated. Per image `N`:

| Field | Required | Notes |
| --- | --- | --- |
| `image_N` | yes | the file. Any field carrying a filename counts; the index is the digits after the last underscore, falling back to its position |
| `used_at_N` | yes | which slot claims it: `ProductImage`, `CategoryImage`, `BrandLogo`, `ReviewImage`, `ReturnPhoto`, `TicketAttachment`, `ProfilePic`, `StoreLogo` |
| `temporary_N` | no | `true`/`false`, default false |

Width, height and the blurhash are derived server-side — do not send them.
Returns `["<image_id>", ...]` in request order. Up to 20 images, 40 megapixels
each. Every image in the batch is validated before any is written, so a 400
leaves nothing behind.

```js
const form = new FormData();
form.append("image_0", file);
form.append("used_at_0", "ProductImage");

const response = await fetch("/image", {
    method: "POST",
    headers: { Authorization: "Bearer " + session.token() },
    body: form,   // no Content-Type — the browser sets the boundary
});
```

### 4. `fetcher.js` has a live bug in its error path

In `#commonMethods`, the `catch` around `response.json()` on a **successful**
response returns `error: error` — but `error` was declared with `let` inside the
earlier `if (!response.ok)` block and is out of scope there. It throws
`ReferenceError` and masks the real failure. Fix before wiring anything that can
return a non-JSON 200.

### Conventions to hold to

- **Errors**: `Fetcher` already toasts on failure. Do not toast again in the
  caller. Pass `showError: false` only when the page renders the error itself
  (inline form validation, an empty state).
- **Successes**: toast only for mutations the user initiated — saved, deleted,
  applied. Never on a page load.
- **Loading**: `skeleton.js` already derives a shimmer from the real subtree.
  Mark the container `data-skeleton` and clear it when the response lands,
  rather than writing spinners.
- **Pagination**: every list endpoint returns `{ items, total, page, limit }`.

---

## Auth — `pages/auth/`

| Page | Calls |
| --- | --- |
| `login.html` | `POST /v1/auth/sign-in`; admin sign-in is a separate route, `POST /v1/auth/admin/sign-in` |
| `register.html` | `POST /v1/auth/sign-up` → `POST /v1/auth/validate-email` (the emailed code) → `POST /v1/auth/resend-verification-code` behind the resend link |
| `forgot-password.html` | `POST /v1/auth/forgot-password` → `POST /v1/auth/verify-reset-code` → `POST /v1/auth/reset-password` |
| nav sign-out | `POST /v1/auth/sign-out`, then clear the token store |

Sign-in is what makes items 1 and 2 above real: it stores the tokens and fires
the cart merge.

---

## Storefront — `pages/shopping/`

### `home.html`
- Category strip → `GET /v1/category/list`
- **Trending Now** → `GET /v1/product/list?sort=best_selling&limit=8`
- **New Arrivals** → `GET /v1/product/list?sort=newest&limit=8`

Both rows map onto the existing sort modes, so no new backend is needed unless
you want a hand-curated slot — there is no merchandising model.

### `listing.html` and `search.html` — `listing.js`
The filters already hold state; they just do not query. Wire the existing
controls to `GET /v1/product/list`, which accepts:

`category` (uuid or slug, matches descendants too), `brand`, `price_min`,
`price_max`, `rating`, `size`, `color`, `tag`, `q`, `sort`
(`relevance | price_asc | price_desc | newest | rating | best_selling`),
`page`, `limit`.

- Sidebar facets: `GET /v1/category/list` and `GET /v1/product/brand/list`.
- Mirror the active filters into the URL query string so a filtered view is
  linkable and the back button works.
- Search is the same endpoint with `q`.

Note: **size and colour have no facet endpoint.** The filter accepts them but
nothing enumerates which sizes and colours exist. Either hardcode the sidebar
list or add a facet endpoint.

### `product.html`
- `GET /v1/product/{slug}` — returns `gallery`, `variants`, `breadcrumb`,
  `related`, and the rating aggregates.
- The gallery is now the variants' images, deduped, in variant order. Picking a
  swatch should jump the gallery to that variant's `image_id`.
- `POST /v1/recently-viewed` on load.
- Reviews → `GET /v1/review/list?product_id=&sort=helpful|newest&page=`
- Write a review → `POST /v1/review`; helpful vote → `POST /v1/review/{id}/vote`
- Add to cart → `POST /v1/cart/item`; wishlist heart → `POST /v1/wishlist`

### `wishlist.html`
`GET /v1/wishlist`, `DELETE /v1/wishlist/{id}`, and add-to-cart per row.
The response carries `price_dropped` against the price when it was added — the
page should show it.

---

## Cart and checkout — `pages/checkout/`

### `cart.html`
- `GET /v1/cart` — returns resolved lines, `subtotal`, `discount`, `total`,
  `item_count`, plus per-line `available`, `in_stock` and `price_changed`. All
  three need rendering; the wireframe shows none of them.
- Quantity → `PATCH /v1/cart/item/{id}`; remove → `DELETE /v1/cart/item/{id}`
- Coupon → `POST /v1/cart/coupon`, `DELETE /v1/cart/coupon`. A coupon that has
  gone stale comes back as `coupon_error` on the cart body rather than as a
  failed request — surface it inline, not as a toast.

### `address.html` — `address.js`
The three selection groups are already interactive; give them data.
- `GET /v1/address/list`, `POST /v1/address`, `PATCH /v1/address/{id}`,
  `DELETE /v1/address/{id}`, `POST /v1/address/{id}/default`
- `GET /v1/checkout/delivery-methods` — prices and day ranges
- `GET /v1/checkout/packaging-options`

### `payment.html`
`GET /v1/payment-method/list`, `POST /v1/payment-method`,
`DELETE /v1/payment-method/{id}`, `POST /v1/payment-method/{id}/default`.

No gateway is wired in — `place_order` records a mock success for everything
except cash on delivery. Do not build a real card form against this yet.

### `review.html` (order review step)
`POST /v1/checkout/place-order` with `address_id`, `billing_address_id`,
`delivery_method_id`, `packaging_option_id`, `gift_message`,
`payment_method_kind`, `payment_method_id`, `customer_note`. Returns the order;
redirect to confirmation with its `uuid`.

Re-price the cart from `GET /v1/cart` on entry — the totals shown here must come
from the server, not from what the cart page happened to calculate.

### `confirmation.html`
`GET /v1/order/{id}` — number, totals, delivery estimate, address snapshots.

---

## Account — `pages/account/`

### `profile.html`
- `GET /v1/account/profile`, `PATCH /v1/account/profile`
- `GET /v1/account/stats` for the order/spend tiles
- Password → `PATCH /v1/auth/change-password`
- Saved addresses and payment methods reuse the checkout endpoints above
- Notification switches → `GET /v1/notification/preferences`,
  `PATCH /v1/notification/preferences`
- Delete account → `POST /v1/account/delete` (raises a deletion request an admin
  actions; it is not an immediate delete — word the confirmation accordingly)

### `orders.html`
- `GET /v1/order/list?status=all|processing|shipped|delivered|cancelled&q=&page=`
- `POST /v1/order/{id}/cancel` — only valid while Pending or Packing; the
  backend rejects anything later, so hide the button past that
- `POST /v1/order/{id}/reorder`
- Returns → `POST /v1/return`, `GET /v1/return/list`

### `tracking.html`
`GET /v1/order/{id}` — carries `tracking` and `shipment_events` for the
timeline.

### `notifications.html`
`GET /v1/notification/list`, `PATCH /v1/notification/read`,
`PATCH /v1/notification/read-all`.

---

## Admin console — `pages/admin/`

All of these need the admin token from `POST /v1/auth/admin/sign-in`; every
handler gates on the `Administrator` role.

### `overview.html`
`GET /v1/admin/dashboard` — returns `revenue`, `orders`, `average_order_value`,
`refund_rate`, a `revenue_series` for the chart, `recent_orders` and
`low_stock`. Takes a `period`.

### `products.html`
- `GET /v1/admin/product/list?tab=all|active|draft|archived|out_of_stock&category=&q=&sort=&page=`
- `PATCH /v1/admin/product/{id}` for the status switch
- `DELETE /v1/admin/product/{id}`
- Stock edits → `POST /v1/admin/product/variant/{id}/stock`

### `product-new.html` — `product-form.js`
- Selects → `GET /v1/admin/category/list`, `GET /v1/product/brand/list`
- Image upload → `POST /image` with `FormData`, **raw fetch, not `Fetcher`**
  (see prerequisite 3). Returns an array of image ids.
- Submit → `POST /v1/admin/product`
- **Each SKU row now requires an `image_id`** — images belong to variants, not
  the product. The form needs a per-row image picker, and the row is invalid
  without one. The product's own cover is derived from the first variant; there
  is no product-level image field any more.
- Editing an existing product: `PATCH /v1/admin/product/{id}` for the fields,
  and the variant routes (`POST /v1/admin/product/{id}/variant`,
  `PATCH|DELETE /v1/admin/product/variant/{id}`) for the rows.

### `categories.html` — `categories.js`
`GET /v1/admin/category/list`, `POST /v1/admin/category`,
`PATCH /v1/admin/category/{id}`, `DELETE /v1/admin/category/{id}`.
The tree's `parent_id` is what makes it two-level.

### `coupons.html` — `coupons.js`
- `GET /v1/admin/coupon/list?tab=all|active|scheduled|expired|used_up|disabled&type=&sort=&page=`
- `POST /v1/admin/coupon`, `PATCH /v1/admin/coupon/{id}`,
  `DELETE /v1/admin/coupon/{id}`, `POST /v1/admin/coupon/{id}/duplicate`
- The page recomputes status from dates and redemption count rather than storing
  it; keep that, and let the server's `used_count` feed it.

### `orders.html`
- `GET /v1/admin/order/list?status=&payment_status=&q=&from=&to=&page=`
- `GET /v1/admin/order/{id}`
- `PATCH /v1/admin/order/{id}/status`
- `POST /v1/admin/order/{id}/tracking`
- `POST /v1/admin/order/{id}/refund` — amount defaults to whatever is
  unrefunded; the server clamps it

### `customers.html`
`GET /v1/admin/customer/list?segment=vip|repeat|new|dormant&location=&page=`,
`GET /v1/admin/customer/{id}`, `PATCH /v1/admin/customer/{id}`.

The page has a search box but this endpoint takes no `q`. Either add one to
`src/handler/admin/customer/list.rs` or drop the box.

### `support.html`
`GET /v1/admin/support/ticket/list?status=&priority=&topic=&q=&page=`,
`GET /v1/admin/support/ticket/{id}`,
`POST /v1/admin/support/ticket/{id}/message`,
`POST /v1/admin/support/ticket/{id}/assign`,
`PATCH /v1/admin/support/ticket/{id}/status`.

### `settings.html`
- `GET /v1/admin/settings/store`, `PATCH /v1/admin/settings/store`
- Shipping zones → `GET /list`, `POST`, `PATCH /{id}`, `DELETE /{id}` under
  `/v1/admin/settings/shipping-zone`
- Staff → `GET /v1/admin/settings/staff/list`,
  `POST /v1/admin/settings/staff/invite`,
  `PATCH /v1/admin/settings/staff/{id}`
- Audit log → `GET /v1/admin/settings/audit-log/list`

---

## Gaps

Things the API surface and the page set do not currently line up on. Worth
deciding before the wiring reaches them.

0. **No admin endpoint for brands.** `/v1/product/brand/list` reads them, but
   nothing creates, edits or deletes one — brands can only be inserted straight
   into Mongo. The admin product form needs a brand picker, so this blocks
   `product-new.html`.
0. ~~**`POST /image` panics rather than returning 400.**~~ Fixed — see
   "Uploading images" below.
1. **No storefront support page.** The customer-side ticket API is complete
   (`/v1/support/ticket` — list, create, detail, reply) and the admin side has a
   page, but there is no customer-facing page to raise a ticket from.
2. **No size/colour facet source** for the listing sidebar (see above).
3. **`admin/customer/list` has no search parameter** though the page has a
   search box.
4. **No entry point to the admin console** from the storefront nav.
5. **Image upload cannot go through `Fetcher`** — different prefix, different
   content type.
6. **Payment is mocked end to end.** Everything except cash on delivery is
   recorded as an immediate success with provider `"mock"`.
