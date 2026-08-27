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
