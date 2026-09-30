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
- [x] **Cart** — lines, the three per-line flags, quantity, coupon, totals.
      `assets/js/cart.js`.
- [x] **Checkout** — address book, delivery, packaging, payment, review and
      confirmation. `assets/js/checkout.js` (shared draft + summary) plus
      `address.js`, `payment.js`, `review.js`, `confirmation.js`.
- [x] **Account** — profile, orders, tracking, notifications, plus the address
      and payment books the checkout shares. `assets/js/account.js` (shared
      sidebar chrome) plus `profile.js`, `orders.js`, `tracking.js`,
      `notifications.js`.
- [x] **Returns and support** — the two complete APIs that had no page at
      all. `assets/js/returns.js`, `assets/js/support.js`, and the two new
      account pages behind them.
- [x] **Reviews** — writing one, and voting a review helpful.
      `assets/js/review-form.js` plus the review list on the product page.
- [x] **Recently viewed** — the row on the home page that reads back what
      every product visit has been writing.
- [x] **Admin console** — all nine pages. `assets/js/admin-core.js` (shared
      chrome and table vocabulary) plus `admin-overview.js`,
      `admin-products.js`, `admin-orders.js`, `admin-customers.js`,
      `admin-support.js`, `admin-settings.js`, and rewrites of
      `categories.js`, `coupons.js` and `product-form.js`.

### What the last of the storefront needed

- **A customer could not write a review.** `POST /v1/review` had been live
  the whole time; the product page's button sent people to their orders and
  the orders page's button was wireframe markup, so the trail dead-ended.
  Both now open the same composer (`review-form.js`), and the product page
  looks up a delivered order of its own accord so the review earns its
  Verified Purchase badge without the customer hunting for the order.
- **`reviewCard` read `review.author_name`.** The list endpoint calls that
  field `author`, so every review on every product was signed "Anonymous".
- **A staff reply on a review was stored and never drawn** — a merchant
  answering a complaint was answering into the void.
- **`POST /v1/recently-viewed` was write-only.** The product page had been
  recording every visit since it was wired and nothing ever read the list
  back. The home page now has the row, built in JS so a visitor with no
  history sees no empty heading.
- **Returns and support both existed end to end with no page.** Two new
  account pages, two new routes, and Returns/Support added to the account
  sidebar on all six pages. The footer's Help column — five dead links on
  every page — now points three of them somewhere real.

### What the admin console needed

Three small backend additions, each because a column or a screen could not
otherwise be honest:

- **`GET /v1/admin/product/{id}`** (new). The console could list and update
  products but not read one back with its variants — the public
  `/v1/product/{slug}` filters on `status: "Active"`, so a Draft, the thing
  most likely to be edited, came back 404.
- **`admin/product/list` now carries `stock_total`, `variant_count` and
  `low_stock`.** `Product` holds only an `in_stock` boolean, and a Stock
  column that can only say yes or no is not a Stock column. One `$in` over
  the variants per page.
- **`admin/order/{id}` now carries `refunded_total` and `refundable`.**
  `refund.rs` works both out from `payment_transaction` before it will
  accept anything, and answers 400 when there is nothing left — but the
  console had no way to know that in advance, so it could only offer a
  refund box and let the operator find out by clicking.

And one correctness fix, the same one as `order/list`: `admin/product/list`
was written `while let Some(Ok(p))`, so a single bad document truncated the
catalogue silently while `count_documents` kept counting it.

Three real defects surfaced only once both ends of a screen existed, and are
fixed:

8. **A staff internal note leaked into the customer's inbox.**
   `admin/support_ticket/reply.rs` wrote `last_message_preview` and bumped
   `message_count` for every reply, internal or not — and the customer-facing
   `GET /v1/support/ticket/list` returns the whole ticket document. So the
   first 140 characters of a staff-only note appeared as the preview line in
   the customer's own ticket list, and their thread claimed more messages
   than `detail.rs` would show them. An internal note now touches none of the
   ticket's rollup fields.

9. **The store had two settings documents, and checkout read the other one.**
   `settings.rs` defines `STORE_SETTINGS_ID` precisely so there is one row —
   but only the console used it. `checkout/quote.rs`, `checkout/place_order.rs`
   and `support_ticket/create.rs` all read `find_one({})`, whichever row Mongo
   handed back. A shop seeded under any other uuid therefore charged 8% tax at
   checkout while the settings screen showed 0%, and the first visit to that
   screen *inserted a second document* which every subsequent save wrote to and
   nothing else ever read. There is now one `Settings::load` helper: it reads
   the canonical id and adopts a legacy row into it rather than starting a
   second, and all four call sites use it.

10. **Every `<input type="date">` was filled from `toISOString()`**, which is
    the UTC day, while every rendered date is local. Where the two disagree the
    edit dialog showed the day before the table did — and saving a coupon
    without touching the field moved its start date back by one. `dateValue()`
    formats the local calendar day.

Decisions worth knowing — every one of these is a control the wireframe drew
that no endpoint can serve, removed or relabelled rather than left to fail on
click:

- **Import CSV** (products), **Create order**, **Print labels**, **New
  ticket** (staff cannot open one as a customer without putting the wrong
  reply-to on the thread), **Email selected**, **Remove customer**, **canned
  responses**, and the **bulk category move** are all gone.
- **Export** buttons everywhere now say *Export CSV* and save what is on
  screen, built in the browser. There is no export endpoint, and a button
  labelled "Export report" promising a server-made report would be a lie.
- **The overview's "↑ 12.4% vs last period" deltas are gone.** Nothing
  returns a previous period. The tiles say which window they cover instead.
- **The Payments settings panel has no switches.** Nothing stores which
  payment methods are enabled, and no processor is connected — the panel now
  says both, and lists what checkout actually accepts.
- **The Notifications settings panel is replaced** by two panels that do have
  endpoints and had no screen: the account-deletion queue and the audit log.
- **Product form: "Track inventory" and "Continue selling at zero" are
  gone** (nothing stores either), and **Brand is a picker, not a text box** —
  `/v1/product/brand/list` is read-only and nothing creates a brand, so a
  typed name could only ever have been discarded.
- **The orders page's "Refunded" tab filters on `payment_status`**, because
  `fulfilment_status` has no Refunded variant — which is what that tab was
  always describing.
- **Bulk actions are loops over the single-item endpoints.** There is no bulk
  endpoint anywhere, so each one confirms with how many requests it is about
  to make, and only ever acts on the page you can see.
- **The customer list's search and sort narrow the current page only.**
  `customer/list` takes `segment`, `location`, `page` and `limit` — no `q`,
  no sort. The placeholder says so; a real one needs the endpoint to grow.
- **Every `.toggle` an admin module drives is marked `data-readonly`**, so
  `toggle.js`'s delegated handler leaves it alone. Two owners flip it twice
  and leave it where it started.

Still unwired, and why:

- `POST /v1/auth/social-login` — the two buttons on the login page need a
  Google or Facebook client SDK to mint the token it takes.
- `GET /v1/auth/user/{email_or_username}` — it answers with a verified
  account's first and last name to anyone who asks. Wiring it into the
  register page as an availability check would turn a leak into a feature.
- `POST /v1/account/fcm-token/add` — mobile push; there is no web client for
  it.
- `/v1/metadata/*` — SEO plumbing, not a screen.
- `POST /v1/payment-method` — needs a processor token. Unchanged.

### What the account pages needed

- **`GET /v1/order/list` now returns each order's items** (title, image,
  quantity). The cards show thumbnails and a unit count, and the list returned
  bare order documents — so drawing four cards meant four extra detail calls.
- **The list silently dropped orders it couldn't deserialise.** `while let
  Some(Ok(o))` stops at the first bad document without a word, while the tab
  counts come from `count_documents` — which is how a page ends up reading
  "All (4)" above one card. It now logs and skips.
- **`toggle.js` respects `data-readonly`**, for switches that show a state
  nothing can change (two-factor).

Decisions worth knowing:

- **Date of birth, gender and language are gone from the profile.** Nothing
  stores them; a Save that accepted and discarded them is worse than not
  offering them. Marketing consent — which is a real field — took their place.
- **Email is read-only.** Changing it means re-verifying it, and there is no
  endpoint for that.
- **Two-factor shows its state but cannot be switched** — `two_factor_enabled`
  is on the account record and nothing exposes a way to set it.
- **The order cards only offer Cancel while the backend will honour it**
  (Pending or Packing); past that the endpoint refuses, and a button that only
  ever errors is worse than no button.
- **Addresses and payment methods got their own sections on the profile**, so
  the sidebar's two settings links go somewhere and the books can be managed
  outside a checkout.
- The three notification switches each own a group of the nine backend flags
  (order+delivery, promotions+digest, price drops+back in stock).

### What checkout needed on the backend

- **`GET /v1/checkout/quote`** (new). The three checkout pages each show a
  running total, and nothing could tell them one: `/v1/cart` knows the
  merchandise and the coupon, but shipping comes from the delivery method and
  tax from the store's rate. The alternative was reimplementing `place_order`'s
  arithmetic in JavaScript, which is the bug this avoids.
- **`utils::pricing::totals`** (new). One pure function for shipping waivers,
  packaging and tax; `place_order` and the quote both call it, so the number
  shown is the number charged.
- **Cart lines now carry `slug`, `brand_name` and `stock`** — the page links
  each line to its product, names the brand, and caps the stepper at what is
  actually left.
- **`coupon::evaluate` printed raw cents** in its minimum-spend message
  ("a minimum spend of 2000").
- The reference data these read — store settings, delivery methods, packaging
  options, coupons — had never been seeded. There is no admin UI for delivery
  methods or packaging either (see gap 7).

Decisions worth knowing:

- **The cart's per-item tick boxes are gone.** `place-order` takes the cart
  whole; there is no way to check out three of five lines, so the boxes
  promised something the backend cannot do.
- **The card form is gone.** `POST /v1/payment-method` takes a
  `provider_token` from a processor's client SDK, and no processor is
  connected — a form posting a PAN to this backend would collect card data the
  system is built not to hold. The page says so. Saved cards, PayPal, Apple Pay
  and cash on delivery all work.
- **The cart shows no shipping or tax**, because neither is decided yet; the
  last line says "Estimated total" and why.
- The checkout draft lives in `sessionStorage` (`bambe.checkout`) and holds
  ids only — every figure is re-fetched from the quote.

Four more pre-existing bugs were fixed on the way through:

6. **An expired access token silently swapped a signed-in cart for an empty
   one.** `fetcher.js` only refreshed after a 401, but the cart, the quote and
   the product page read the caller through `optional_access`, which treats an
   unverifiable token as no token and answers 200 with a guest's empty cart —
   and the next add-to-cart minted a fresh anonymous cart while the real one
   sat there. It now refreshes before sending when the token has expired.
7. **The guest cart was never merged on sign-in.** `completeSignIn` calls
   `POST /v1/cart/merge` with the guest id in a header and no body — but the
   handler takes `web::Json<ReqBody>`, which rejects a bodyless request, so
   every merge 400'd behind `showError: false`. It sends `session_id` in the
   body now.

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
   recorded as an immediate success with provider `"mock"`. Nothing can create
   a saved card either, because that needs a processor's tokeniser — so the
   payment step offers saved cards, PayPal, Apple Pay and cash on delivery, and
   says plainly why a new card can't be added.
7. **Nothing ever writes a notification.** The list, read, read-all and
   preference endpoints are all complete, and no handler inserts a row — so
   placing an order, shipping it or dropping a wishlisted item's price produces
   nothing. The page was verified against seeded rows. `place_order`,
   `admin/order/status` and the price-drop path are where they belong.
8. **No endpoint to change an email, toggle two-factor, or upload an avatar.**
   All three are fields on the account; none has a route.
9. **No admin screen for delivery methods or packaging options.** Both are
   required to check out and both can only be inserted straight into Mongo.
   `settings.html` covers shipping *zones*, which is a different collection
   that nothing reads at checkout.

---

## Closing the backlog (2026-09-01)

Everything below was on the "what's left" list and is now done and verified
against a running server. The one item deliberately untouched is the payment
gateway, which needs a processor chosen first — see the end.

### Correctness

- **Stock no longer oversells.** `place_order` took stock with a
  read-then-`$set`, so two checkouts racing for the last unit both wrote the
  same total and both shipped. It now reserves with `$inc` guarded by
  `stock: { $gte: qty }` — the test and the decrement are one atomic
  operation, and the loser matches no document. The same read-modify-write
  was in `order/cancel`, `admin/order/update_status` and
  `admin/product/adjust_stock`; all four use `find_one_and_update` now, and
  the movement log records the balance the write returned rather than one
  computed beforehand. `adjust_stock` also refuses a write-off larger than
  the stock on hand, which a `$set` could not express.

  Reservation happens *before* the order document is written, and every
  failure path after it releases what it took. **Verified**: two concurrent
  checkouts for a variant with one unit — one 200, one 400, stock 0 (not −1),
  one order created.

- **Order numbers are a counter, not a dice roll.** `random_range(10_000..99_999)`
  with no uniqueness check collides about half the time by ~350 orders a
  year. `utils::counter::next` advances a per-year counter with one `$inc`
  upsert and returns the post-increment value, so concurrent checkouts cannot
  draw the same sequence. Drawn after the stock reservation, so a sold-out
  attempt doesn't burn a number.

- **`refunded_quantity` finally has a writer.** It was initialised to 0, read
  in three places, and incremented nowhere — so a customer could open a return
  for the same line over and over. Two halves:

  1. `return_request/create` now counts what is already claimed by any
     non-rejected return on the order, not just what has been refunded.
     Between filing and resolution a line is spoken for.
  2. `admin/return_request/resolve` (new) writes `refunded_quantity` and
     `refunded_amount` when a return is refunded.

- **The auth transactions actually roll back now.** `.session(...)` appeared
  zero times in the codebase, and the mongodb Rust driver requires it on each
  operation for that operation to join the transaction — so
  `start_transaction` / `abort_transaction` wrapped operations that were never
  enrolled. 31 operations across the seven auth handlers are enrolled.
  **Verified**: a sign-up whose email send fails (Resend rejects
  `example.com`) used to leave its `account_core` row behind; all three
  inserts now roll back and the collections come back empty.

- **One Mongo client for the process.** `MongoDB.connect()` built a fresh
  `Client` per call and every handler calls it, so each request stood up its
  own pool (`min_pool_size: 8`) and dropped it. A `OnceLock` behind the same
  signature fixes it without touching a call site.

- **Cursor loops log instead of truncating.** 77 `while let Some(Ok(x))` loops
  across 45 files ended silently on the first undeserialisable document while
  `count_documents` kept counting it. Each now skips the row and logs it with
  `module_path!()`, so the log names the list that dropped something.

- **Indexes exist.** `builtins/indexes.rs` creates the uniqueness constraints
  that were documented in model comments and enforced nowhere — `product.slug`,
  `order.number`, `coupon.code`, `payment_transaction.provider_transaction_id`,
  `(coupon_id, order_id)` on redemptions, `(review_id, user_id)` on votes —
  plus 18 lookup indexes behind filters that were collection-scanning. Unique
  indexes are sparse so pre-existing documents without the field don't collide
  on a shared `null`. Idempotent, run at startup, failures logged not fatal.

- **Coupon fixes.** `first_order_only` counted orders with
  `payment_status: "Paid"`, which cash-on-delivery never reaches until the
  courier collects — so a COD customer reused a first-order coupon forever. It
  now counts any non-cancelled order. `value` is validated on create *and*
  update (a percentage over 10000 bps or a negative one produced a discount
  bigger than the basket, and `place_order` has no floor); the update checks
  the values the coupon will end up with, since changing type without touching
  `value` reinterprets it. Cancelling an order now deletes the redemption and
  decrements `used_count`, so a cancelled order stops burning the per-user
  limit.

- **An unresolvable `billing_address_id` is a 400**, not a silent fall back to
  the shipping address under a different address's id.

### Returns, end to end

Returns were a dead end: the customer could file one and nothing could act on
it. Three endpoints and a screen close that.

- `GET /v1/admin/return/list` — status filter, order-number search, paging,
  and an `open` count for the badge.
- `GET /v1/admin/return/{id}` — joins the return lines to the order lines for
  title, price and what's already refunded.
- `PATCH /v1/admin/return/{id}` — the status move. Refunded is terminal and
  the endpoint refuses a second attempt; rejected can't be reopened.

Refunding marks the order lines, restocks (unless staff say the goods came
back damaged), writes the refund transaction, flips the order to `Returned`
once every unit is back, and mails the customer.

**The refund ceiling is what was captured, not what the order is worth.** An
uncollected cash-on-delivery order has no Sale transaction, so nothing is
given back — the lines are still marked and the stock still returns. The
detail endpoint computes the same ceiling, so the dialog can't offer an amount
the server will clamp away; it says "no payment was ever captured" instead.

`pages/admin/returns.html` + `admin-returns.js` are the screen, with Returns
added to the sidebar of all ten admin pages and a badge showing the open
queue.

### Mail and notifications

`notification` had list / read / read-all / preferences and **no writer at
all**, so the bell was permanently empty. `utils/notify.rs` is that writer and
the single place that decides whether a user wants a given message: a topic
goes out only where its own preference flag allows it, and email additionally
needs `email_enabled`. The in-app row is not a channel — it *is* the list the
bell reads — so only the topic gates it. Defaults match the `None` branch of
`get_preferences`, which had always promised "the defaults the sender
applies".

Wired at: order placed, shipped, delivered, cancelled (both the customer's own
cancel and the console's), refund issued, return resolved, and support reply.
A staff **internal note** returns before the notify, so it still can't reach
the customer. A guest ticket has no account to hang a notification on and is
skipped.

Six order-lifecycle email templates were added to `resend.rs` on the same
`shell` the auth codes use. All mail is Resend now; `smtp.rs` and `lettre` are
gone.

**Verified**: placing an order produced the row *and* the email; shipping and
delivering each produced theirs; resolving a return produced its own. No mail
errors in the log.

### Delivery methods and packaging

The two collections checkout actually charges from had no admin endpoint at
all — the only way to add or reprice one was to insert it into Mongo by hand.
`settings.html` covered shipping *zones*, a different collection nothing reads
at checkout, and the panel had a footnote apologising for it.

Eight endpoints under `/v1/admin/settings/delivery-method/*` and
`/v1/admin/settings/packaging-option/*`, and two panels on the Shipping tab
above the zones. Prices and delivery windows are validated on both create and
update, and **the last enabled delivery method can't be disabled or deleted** —
checkout cannot quote a price without one, and that failure would show up as
an empty delivery step rather than as anything pointing back at settings.

**Verified**: an inverted window and a negative price are both refused; a
created method appears at `/v1/checkout/delivery-methods` and vanishes when
disabled; the last-method guard fires on both disable and delete.

### Still not done

- ~~The payment gateway.~~ **Done — SSLCommerz**, see below.
- **Social login** needs a Google or Facebook client SDK to mint the token
  `/v1/auth/social-login` takes.
- **Brand CRUD** endpoints.
- **No test suite, CI, or seed script.** Two `#[cfg(test)]` modules in the
  whole crate; `utils/password.rs` carries the second.
- The home page still has no merchandising model behind its hero and promo
  copy.
- Product images in the database are 8×8 placeholder stubs, so every card
  renders as a flat colour block. The upload pipeline itself works — the
  images just need replacing through the admin product form.

### Passwords

Stored as Argon2id PHC strings. `utils/password.rs` is the only module that
touches a stored password: handlers call `hash` on the way in and `verify` on
the way out, and never compare two password strings themselves.

This replaced plaintext storage. `Handler::Auth::migrate_plaintext_passwords`
runs at startup and hashes any row still held the old way — it can do that
without anyone signing in, because a plaintext row *is* the password. It is
idempotent and safe to interrupt.

`verify` still accepts a non-PHC stored value, comparing it literally and
returning `Verified::NeedsRehash`; `sign_in.rs` and `admin_sign_in.rs` upgrade
the row when they see it. That path exists only so an interrupted migration
cannot lock anyone out, and is expected never to fire once the migration has
completed. **Accounts with an empty stored password — the ones
`social_login.rs` creates — never authenticate by password**, including
against an empty attempt.

### Creating an administrator

There is no UI. `POST /api/v1/auth/admin/sign-up` with a `secret_key` matching
`ADMIN_SIGNUP_SECRET`, which is the endpoint's only access control: it takes no
token, so an existing admin is not required to create another. The handler
refuses to run when that variable is unset, shorter than 24 characters, or
still the literal example value.

Admins live in `admin_account`, separate from `account_core`, so the same email
may exist once as an admin and once as a buyer. There is no email verification
step and no 2FA on the admin path.

### The payment gateway (SSLCommerz)

The mock is gone. **No code path on this server can mark an order paid on its
own say-so.**

`place_order` writes every order unpaid. Cash on delivery stays `Pending`
until the courier collects; everything else gets an SSLCommerz session and the
browser is redirected to their hosted page, so no card number ever reaches
this server. Only a validated callback flips an order to `Paid` and writes its
`Sale` row — and `payment_transaction` is the ledger every refund path sums,
so a row in it now means money actually moved.

**Callbacks are not believed.** All four arrive unauthenticated — anyone can
POST `status=VALID` at `/payment/sslcommerz/ipn`. The only field taken from
the body is `val_id`, which is presented to SSLCommerz's validation API over a
server-to-server call authenticated with the store password; the status,
amount and currency that come back from *that* are what decide anything. Three
further checks: the validation's own `tran_id` must match the callback's (so a
valid `val_id` from one order can't be replayed against another), the amount
must cover the order's `grand_total` in the order's currency, and the flip to
Paid is a conditional update on `payment_status: "Pending"` — two callbacks
racing both reach it and only one matches, so the Sale row cannot be written
twice. SSLCommerz sends both a browser redirect and a server IPN with no
promise about ordering, so that idempotency is load-bearing rather than
defensive.

A declined, cancelled or unverifiable payment releases the order:
`utils::order::release_order_holds` puts the stock back with an atomic `$inc`,
logs the movement, and hands back the coupon redemption. That helper is now
shared with both cancel paths, which had grown their own copies.

Configuration is four environment variables — `SSLCZ_STORE_ID`,
`SSLCZ_STORE_PASSWD`, `SSLCZ_SANDBOX` (defaults true), and `SITE_URL` for the
callback base. **With none of them set the shop degrades rather than breaks**:
`GET /v1/checkout/quote` reports `card_payment_available: false`, the payment
step offers only cash on delivery and says why, and `place_order` refuses a
card outright instead of redirecting into a 500.

What the checkout step lost, because nothing behind it was real: PayPal and
Apple Pay (no route sent them anywhere), and the saved-card rows. SSLCommerz's
hosted checkout returns no reusable token, so picking a saved card would send
the shopper to the same page to type the same number having implied otherwise.
The rows stay on the profile, where they are a record rather than a promise.
One option now covers cards, mobile banking and net banking, because that is
what one SSLCommerz redirect actually is.

**Verified against the real sandbox**, not mocked:

- Unconfigured: `card_payment_available: false` and a card order is refused
  with a message pointing at cash on delivery.
- Configured: a card order came back with a genuine
  `https://sandbox.sslcommerz.com/EasyCheckOut/...` URL. The first attempt was
  refused by SSLCommerz for a missing `ship_postcode`, which is exactly the
  kind of thing a mock would never have caught — `cus_state`, `cus_postcode`,
  `ship_state` and `ship_postcode` are all sent now.
- A forged IPN (`status=VALID`, `amount=99999.00`) left the order Pending with
  no Sale row, and logged why.
- A forged success callback with a plausible `val_id` was answered
  `INVALID_TRANSACTION` by the live validator; the order was released — stock
  back, cancellation movement logged, redirected to
  `/checkout/payment?error=verification`.
- Cash on delivery: order `Pending`, `paid_at` null, no Sale row.

Still open on payments: `POST /v1/payment-method` cannot create a card,
because that needs a tokeniser this integration doesn't use; and there is no
"pay now" action on an order left unpaid by an abandoned session, only
`POST /v1/payment/abandon` to cancel one.

### Two-step sign-in and the avatar (2026-09-01)

**Changing an email address is deliberately not offered.** An address is where
password reset and the two-step code are sent, so changing it means
re-verifying it, and the product doesn't do that. The field stays read-only by
decision, not omission — it is not a gap waiting to be filled.

**Two-factor turned out to be broken, not just unwired.** `sign_in.rs` checked
`two_a_factor_auth_enabled`, wrote a `sign_in_verification_request`, emailed
the code — and then **fell straight through to minting the access token
anyway**. There was also no endpoint to submit the code, so the request row was
written and nothing could consume it. Enabling the flag bought a spare email
and no second factor at all. It went unnoticed because nothing could set the
flag; adding the switch is what made it reachable.

Three parts:

- `PATCH /v1/account/two-factor` — the writer that never existed. Turning it
  **on** is free; turning it **off** costs the current password, because an
  attacker holding a live token must not be able to strip a factor off the
  account quietly. Enabling is refused on an unverified address, which would
  otherwise send the code somewhere the owner hasn't proven they can read.
  Either direction sends a Security notification.
- `sign_in.rs` now **returns** at the second factor — the request id and no
  token.
- `POST /v1/auth/verify-sign-in-code` exchanges the id and the code for the
  session. The request is deleted before the token is minted, so one code buys
  one session and a replay finds nothing. An unknown id and a wrong code give
  the same 403, so neither can be probed to learn about the other, and
  suspension is re-checked between the two steps.

The login page swaps the password form for a code field in place rather than
opening a dialog — same act of signing in — and clears the password so a stale
value can't be resubmitted from behind it.

**The avatar needed no new endpoint.** `ProfilePic` was already a valid image
slot and `update_profile` already accepted `profile_picture`; what was missing
was the picker. The pencil on the profile hero now uploads through `POST
/image` (raw fetch — multipart, outside `/api`, no explicit Content-Type) and
PATCHes the returned uuid, with a 5 MB and image-type check before the upload.

**Verified end to end:** sign-in withholds the token with 2FA on; a wrong code,
an unknown request id and a replay of a spent code all give the same 403; the
correct code mints a session; the login page's code step appears, hides the
password form and preserves the sign-up link; the toggle refuses to switch off
on a wrong password and does nothing on a cancelled prompt; and an uploaded
avatar renders from `/image/webp/{id}` on the profile hero.
