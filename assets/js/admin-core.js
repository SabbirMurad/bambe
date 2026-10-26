/* ═══════════════════════════════════════════════════════════════
   Bambee Web — admin-core.js

   The furniture every admin page carries, and the small vocabulary
   the nine page modules share: the guard, the signed-in identity,
   sign-out, the sidebar counts, and the table / pager / status-tag
   helpers each screen would otherwise reinvent.

   `admin.js` stays as it is — it wires the wireframe's tabs and row
   checkboxes and knows nothing about data. This file is everything
   that talks to `/v1/admin/*`.

   One rule runs through all nine screens: a control is only drawn
   when the endpoint behind it exists. The console has 46 endpoints
   and the wireframe drew rather more than 46 buttons, so the
   difference is either removed or labelled as unavailable, never
   left to fail on click.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { formatMoney, currencySymbol } from './currency.js';
import './nav-search.js';
import { session } from './session.js';

export { Fetcher, toast, session };
export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];

/* ── Money and time ───────────────────────────────────────────
   Cents in, string out — the same rule as the storefront, kept
   here so an admin module never has to import from catalog.js and
   drag the product card in behind it.                          */
export function money(cents) {
  if (cents === null || cents === undefined) return '—';
  /* formatMoney rounds first: most figures arrive as whole cents, but
     the aggregates do not — `avg_lifetime_value` is a Mongo `$avg` and
     comes back as 7721.333…, which once printed as "$77.21.33333333333". */
  return formatMoney(cents);
}

export function shortMoney(cents) {
  const units = (cents || 0) / 100;
  if (Math.abs(units) >= 1000) return currencySymbol() + (units / 1000).toFixed(1) + 'k';
  return currencySymbol() + units.toFixed(0);
}

export function fullDate(ms) {
  if (!ms) return '—';
  return new Date(ms).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

export function dateTime(ms) {
  if (!ms) return '—';
  return new Date(ms).toLocaleString(undefined, {
    year: 'numeric', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
  });
}

/*
  The value an <input type="date"> wants, in the *local* calendar day.

  `toISOString().slice(0, 10)` is the UTC day, and every other date on
  these screens is rendered local. Where the two disagree — anywhere the
  local clock is not on the same date as UTC — the table said one day and
  the edit dialog said the day before, and saving without touching the
  field silently moved a coupon's start date back by one.
*/
export function dateValue(ms) {
  if (!ms) return '';
  const date = new Date(ms);
  const pad = (n) => String(n).padStart(2, '0');
  return date.getFullYear() + '-' + pad(date.getMonth() + 1) + '-' + pad(date.getDate());
}

/* "2 min ago" up to a week, then a date — the recent-orders table
   reads as a feed, and "18 Aug" tells you nothing about whether an
   order just landed. */
export function ago(ms) {
  if (!ms) return '';
  const seconds = Math.round((Date.now() - ms) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return minutes + ' min ago';
  const hours = Math.round(minutes / 60);
  if (hours < 24) return hours + (hours === 1 ? ' hr ago' : ' hrs ago');
  const days = Math.round(hours / 24);
  if (days <= 7) return days === 1 ? 'yesterday' : days + ' days ago';
  return fullDate(ms);
}

/* ── Status tags ──────────────────────────────────────────────
   The stylesheet has four tag colours and the models have rather
   more states than that, so each family maps its own.          */
const ORDER_TAG = {
  Pending: 'tag-gold',
  Packing: 'tag-blue',
  OnHold: 'tag-gold',
  Shipped: 'tag-green',
  Delivered: 'tag-green',
  Cancelled: 'tag-red',
  Returned: 'tag-red',
};

const PAYMENT_TAG = {
  Pending: 'tag-gold',
  Paid: 'tag-green',
  Refunded: 'tag-red',
  PartiallyRefunded: 'tag-gold',
  Failed: 'tag-red',
};

const TICKET_TAG = {
  Open: 'tag-gold',
  Pending: 'tag-blue',
  Resolved: 'tag-green',
  Closed: 'tag-red',
};

export function tag(text, className) {
  const el = document.createElement('span');
  el.className = 'tag ' + (className || 'tag-blue');
  el.textContent = text;
  return el;
}

export function orderTag(status) { return tag(spaced(status), ORDER_TAG[status] || 'tag-blue'); }
export function paymentTag(status) { return tag(spaced(status), PAYMENT_TAG[status] || 'tag-blue'); }
export function ticketTag(status) { return tag(spaced(status), TICKET_TAG[status] || 'tag-blue'); }

/** "PartiallyRefunded" → "Partially refunded". The enums are PascalCase. */
export function spaced(value) {
  if (!value) return '';
  return String(value)
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/^(.)/, (c) => c.toUpperCase())
    .replace(/ (.)/g, (m, c) => ' ' + c.toLowerCase());
}

/* ── Tables ───────────────────────────────────────────────────  */

export function cell(content, className) {
  const td = document.createElement('td');
  if (className) td.className = className;
  if (content instanceof Node) td.appendChild(content);
  else if (content !== undefined && content !== null) td.textContent = content;
  return td;
}

/** A two-line cell: a strong value with a quieter line under it. */
export function stacked(main, sub, mainClass = 'cell-strong') {
  const wrap = document.createElement('div');
  const top = document.createElement('div');
  top.className = mainClass;
  top.textContent = main;
  wrap.appendChild(top);
  if (sub) {
    const bottom = document.createElement('div');
    bottom.className = 'cell-sub';
    bottom.textContent = sub;
    wrap.appendChild(bottom);
  }
  return wrap;
}

/** Buttons for a row's action cell, in the wireframe's own shape. */
export function rowButton(label, onClick, { danger = false, title } = {}) {
  const el = document.createElement('span');
  el.className = 'row-btn' + (danger ? ' danger' : '');
  el.tabIndex = 0;
  el.textContent = label;
  if (title) el.title = title;
  el.addEventListener('click', (e) => { e.stopPropagation(); onClick(e); });
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); onClick(e); }
  });
  return el;
}

/**
 * Replaces a tbody's rows, or fills it with a single message row.
 * Returns true when there was something to draw.
 */
export function fillTable(tbody, rows, { columns, empty = 'Nothing here yet' } = {}) {
  if (window.Skeleton) window.Skeleton.clear(tbody);
  tbody.innerHTML = '';

  if (!rows.length) {
    const tr = document.createElement('tr');
    const td = document.createElement('td');
    td.colSpan = columns || tbody.closest('table').querySelectorAll('thead th').length || 1;
    td.style.cssText = 'padding:36px 16px;text-align:center;color:var(--subtle);font-size:13px;';
    td.textContent = empty;
    tr.appendChild(td);
    tbody.appendChild(tr);
    return false;
  }

  const frag = document.createDocumentFragment();
  rows.forEach((row) => frag.appendChild(row));
  tbody.appendChild(frag);
  return true;
}

/* ── Paging ───────────────────────────────────────────────────
   The wireframe drew a `.pagination` under each table with three
   fixed page buttons. Same markup, real numbers.               */
/** "Showing 21–40 of 312 orders" under a paged table, from the list's own totals. */
export function paintShowing(el, { total, limit, page }, noun) {
  if (!el) return;
  if (!total) { el.textContent = 'No ' + noun; return; }
  const from = (page - 1) * limit + 1;
  const to = Math.min(page * limit, total);
  el.textContent = 'Showing ' + from + '–' + to + ' of ' + total.toLocaleString() + ' ' + noun;
}

export function paintPager(pager, { total, limit, page }, onPage) {
  if (!pager) return;
  const pages = Math.ceil((total || 0) / (limit || 20));

  pager.innerHTML = '';
  pager.style.display = pages > 1 ? '' : 'none';
  if (pages <= 1) return;

  const step = (target, label, disabled) => {
    const el = document.createElement('div');
    el.className = 'page-btn' + (target === page ? ' active' : '') + (disabled ? ' disabled' : '');
    el.textContent = label;
    if (!disabled && target !== page) el.addEventListener('click', () => onPage(target));
    return el;
  };

  pager.appendChild(step(page - 1, '‹', page === 1));

  /* A window around the current page rather than every page — 400
     products is 20 pages and a row of 20 buttons is not navigation. */
  const from = Math.max(1, page - 2);
  const to = Math.min(pages, from + 4);
  for (let n = from; n <= to; n++) pager.appendChild(step(n, String(n)));

  pager.appendChild(step(page + 1, '›', page === pages));
}

export function debounce(fn, wait = 300) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), wait);
  };
}

/* ── Chrome ───────────────────────────────────────────────────  */

let me = null;
export function currentAdmin() { return me; }

/*
  The sidebar counts were hardcoded (24 orders, 318 products, 1,204
  customers, 7 tickets). They are cheap to make true: every one of
  those lists already returns a `total`, and the support one is the
  only number an operator acts on, so it keeps its alert styling
  when there is anything open.
*/
async function paintNavCounts() {
  const items = {};
  $$('.admin-nav-item').forEach((item) => {
    const count = item.querySelector('.admin-nav-count');
    if (!count) return;
    const label = item.textContent.trim().toLowerCase();
    if (label.startsWith('orders')) items.orders = count;
    else if (label.startsWith('products')) items.products = count;
    else if (label.startsWith('categories')) items.categories = count;
    else if (label.startsWith('customers')) items.customers = count;
    else if (label.startsWith('coupons')) items.coupons = count;
    else if (label.startsWith('support')) items.support = count;
    else if (label.startsWith('returns')) items.returns = count;
    else if (label.startsWith('reviews')) items.reviews = count;
  });

  const set = (el, value) => {
    if (!el) return;
    el.textContent = (value || 0).toLocaleString();
    el.style.display = value ? '' : 'none';
  };

  const [orders, products, categories, customers, coupons, tickets, returns, reviews] = await Promise.all([
    Fetcher.get({ endpoint: '/v1/admin/order/list', query: { limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/product/list', query: { limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/category/list', showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/customer/list', query: { limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/coupon/list', query: { limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/support/ticket/list', query: { status: 'open', limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/return/list', query: { limit: 1 }, showError: false }),
    Fetcher.get({ endpoint: '/v1/admin/review/list', query: { limit: 1 }, showError: false }),
  ]);

  set(items.orders, orders.ok ? orders.data.total : 0);
  set(items.products, products.ok ? products.data.total : 0);
  set(items.categories, categories.ok ? (categories.data || []).length : 0);
  set(items.customers, customers.ok ? customers.data.total : 0);
  set(items.coupons, coupons.ok ? coupons.data.total : 0);
  set(items.reviews, reviews.ok ? reviews.data.total : 0);

  /* The badge is the open queue, not the lifetime total — a count of
     every return ever filed tells staff nothing about today. */
  set(items.returns, returns.ok ? ((returns.data.stats && returns.data.stats.open) || 0) : 0);
  if (items.returns) {
    items.returns.classList.toggle('alert',
      returns.ok && ((returns.data.stats && returns.data.stats.open) || 0) > 0);
  }

  const open = tickets.ok ? tickets.data.total : 0;
  set(items.support, open);
  if (items.support) items.support.classList.toggle('alert', open > 0);
}

/*
  Call at the top of every admin page module. Returns the admin
  record, or null when the visitor is being bounced — in which case
  the caller must stop.

  The guard is `/v1/admin/me` rather than the token's own role
  claim: a token can say Administrator and still be for an account
  that has since been removed, and every other endpoint on the page
  would then 403 one by one.
*/
export async function mountAdminChrome() {
  if (!session.isSignedIn()) {
    const next = encodeURIComponent(location.pathname + location.search);
    location.href = '/admin/login?next=' + next;
    return null;
  }

  const response = await Fetcher.get({ endpoint: '/v1/admin/me', showError: false });
  if (!response.ok) {
    // 401/403 both mean "not an administrator here" as far as the
    // console is concerned.
    if (response.status === 401 || response.status === 403) {
      const next = encodeURIComponent(location.pathname + location.search);
      location.href = '/admin/login?next=' + next;
    } else {
      toast.error("Couldn't verify your admin session");
    }
    return null;
  }

  me = response.data;

  const name = $('.nav-user-name');
  if (name) name.textContent = me.name || 'Admin';

  /*
    The console's own bell, over `/v1/admin/notification/*` — a feed
    shared by the whole team (staff live in `admin_account`, so the
    shopper's `/v1/notification/list` can't serve it). Today it
    carries the loyalty alerts: a customer one stamp from a full card,
    and a full card with no reward set.
  */
  mountAdminBell();

  const logout = $('.admin-nav-logout');
  if (logout) {
    logout.onclick = async () => {
      await Fetcher.post({ endpoint: '/v1/auth/sign-out', body: { refresh_token: session.refreshToken() }, showError: false });
      session.clear();
      location.href = '/admin/login';
    };
  }

  /* The console's own search box spans three collections and there
     is no endpoint that does that, so it routes to whichever list
     can answer rather than pretending to be global. */
  wireConsoleSearch();
  paintNavCounts();

  return me;
}

/* ── Notification bell ────────────────────────────────────────  */

const BELL_SVG = '<svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M10 2.5a5 5 0 0 1 5 5c0 4.2 1.6 5.5 1.6 5.5H3.4S5 11.7 5 7.5a5 5 0 0 1 5-5ZM8 16a2 2 0 0 0 4 0" stroke="#32302C" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>';
const BELL_POLL_MS = 60000;

function mountAdminBell() {
  const actions = $('.nav-actions');
  if (!actions || $('.admin-bell')) return;

  const wrap = document.createElement('div');
  wrap.className = 'admin-bell';

  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'nav-icon-btn';
  button.setAttribute('aria-label', 'Notifications');
  button.setAttribute('aria-expanded', 'false');
  button.innerHTML = BELL_SVG + '<div class="nav-badge"></div>';
  wrap.appendChild(button);

  const panel = document.createElement('div');
  panel.className = 'admin-bell-panel';
  panel.hidden = true;
  wrap.appendChild(panel);
  actions.insertBefore(wrap, actions.firstChild);

  let items = [];

  const paintPanel = () => {
    panel.innerHTML = '';
    const head = document.createElement('div');
    head.className = 'admin-bell-head';
    const title = document.createElement('span');
    title.textContent = 'Notifications';
    head.appendChild(title);
    if (items.some((n) => !n.read_at)) {
      const all = document.createElement('button');
      all.type = 'button';
      all.className = 'admin-bell-all';
      all.textContent = 'Mark all read';
      all.onclick = async () => {
        await Fetcher.post({ endpoint: '/v1/admin/notification/read-all', body: {}, showError: false });
        refresh();
      };
      head.appendChild(all);
    }
    panel.appendChild(head);

    if (!items.length) {
      const empty = document.createElement('div');
      empty.className = 'admin-bell-empty';
      empty.textContent = 'Nothing needs you right now.';
      panel.appendChild(empty);
      return;
    }

    items.forEach((n) => {
      const row = document.createElement('button');
      row.type = 'button';
      row.className = 'admin-bell-item' + (n.read_at ? '' : ' is-unread');
      const t = document.createElement('div');
      t.className = 'admin-bell-title';
      t.textContent = n.title;
      const b = document.createElement('div');
      b.className = 'admin-bell-body';
      b.textContent = n.body;
      const when = document.createElement('div');
      when.className = 'admin-bell-when';
      when.textContent = ago(n.created_at);
      row.append(t, b, when);
      row.onclick = async () => {
        if (!n.read_at) {
          await Fetcher.post({ endpoint: '/v1/admin/notification/' + n.uuid + '/read', body: {}, showError: false });
        }
        if (n.link) location.href = n.link;
        else refresh();
      };
      panel.appendChild(row);
    });
  };

  const refresh = async () => {
    const response = await Fetcher.get({ endpoint: '/v1/admin/notification/list', query: { limit: 20 }, showError: false });
    if (!response.ok) return;
    items = response.data.items || [];
    const unread = response.data.unread || 0;
    button.querySelector('.nav-badge').textContent = unread ? (unread > 99 ? '99+' : String(unread)) : '';
    button.setAttribute('aria-label', unread ? 'Notifications, ' + unread + ' unread' : 'Notifications');
    if (!panel.hidden) paintPanel();
  };

  const close = () => {
    panel.hidden = true;
    button.setAttribute('aria-expanded', 'false');
  };
  button.onclick = (event) => {
    event.stopPropagation();
    if (panel.hidden) {
      paintPanel();
      panel.hidden = false;
      button.setAttribute('aria-expanded', 'true');
    } else {
      close();
    }
  };
  document.addEventListener('click', (event) => { if (!wrap.contains(event.target)) close(); });
  document.addEventListener('keydown', (event) => { if (event.key === 'Escape') close(); });

  refresh();
  setInterval(() => { if (!document.hidden) refresh(); }, BELL_POLL_MS);
}

function wireConsoleSearch() {
  const input = $('.nav-search input');
  const button = $('.nav-search-btn');
  if (!input) return;

  input.placeholder = 'Search orders, products, customers…';

  const go = () => {
    const q = input.value.trim();
    if (!q) return;
    // Order numbers are the one unambiguous thing typed in here.
    const target = /^#?[A-Za-z]{2}-/.test(q) ? '/admin/orders' : '/admin/products';
    location.href = target + '?q=' + encodeURIComponent(q.replace(/^#/, ''));
  };

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') go(); });
  if (button) button.onclick = go;
}

/* ── Image upload ─────────────────────────────────────────────
   `POST /image` is the one endpoint outside the `/api` scope, and
   it takes multipart — `Fetcher` hardcodes application/json, so
   this is a raw fetch. The response is a bare array of the new
   image uuids, in the order they were sent.

   Field names are indexed (`image_0`, `used_at_0`): the handler
   reads the digits after the last underscore, and `used_at_N` is
   the only metadata it insists on.                              */
export async function uploadImage(file, usedAt = 'ProductImage') {
  const form = new FormData();
  form.append('image_0', file);
  form.append('used_at_0', usedAt);

  const headers = {};
  const token = session.token();
  if (token) headers.Authorization = 'Bearer ' + token;

  let response;
  try {
    response = await fetch((window.projectDomain || '') + '/image', {
      method: 'POST',
      headers,                 // no Content-Type — the browser sets the boundary
      body: form,
    });
  } catch (error) {
    toast.error('Upload failed: ' + error);
    return null;
  }

  if (!response.ok) {
    let message = 'Upload failed';
    try {
      const body = await response.json();
      message = body.message || body.error || message;
    } catch (e) { /* a non-JSON error body is still a failed upload */ }
    toast.error(message);
    return null;
  }

  const ids = await response.json();
  return Array.isArray(ids) ? ids[0] : null;
}

/* ── Confirmations ────────────────────────────────────────────
   Re-exported so an admin module imports one file, not two.    */
export { openModal, confirmModal, field, textInput, textArea, selectInput } from './modal.js';
