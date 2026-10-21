/* ═══════════════════════════════════════════════════════════════
   Bambee Web — admin-reviews.js

   The staff side of reviews, against `/v1/admin/review/*`.

   Only customers with a delivered order can review, and a review goes
   live the moment it is written — so this page is for after the
   fact: hide one that shouldn't be up, answer one publicly, or
   delete one outright.

   A whole-order review is stored as one row per product, but it is
   one review to the customer, so it is listed once with every
   product it sits on, and every action here applies to all of them.

   Hide vs delete: a hidden review still counts as written, so the
   customer can't just post it again. Deleting removes it and its
   helpful votes, and lets the customer review those products anew.
   Both recompute the products' star ratings.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, dateTime, ago, debounce, tag,
  cell, rowButton, fillTable, paintPager, mountAdminChrome,
  openModal, confirmModal, field, textArea,
} from './admin-core.js';

const TABS = ['all', 'published', 'hidden'];

let state = { status: 'all', rating: '', q: '', page: 1, limit: 20 };

function starText(rating) {
  return '★'.repeat(rating) + '☆'.repeat(5 - rating);
}

function stars(rating) {
  const span = document.createElement('span');
  span.style.cssText = 'color:#C8A772;letter-spacing:1px;margin-right:6px;';
  span.textContent = starText(rating);
  span.setAttribute('aria-label', rating + ' out of 5 stars');
  return span;
}

function statusTag(status) {
  return status === 'Published' ? tag('Published', 'tag-green') : tag('Hidden', 'tag-red');
}

function productLabel(review) {
  const first = review.products[0];
  if (!first) return '—';
  return review.products.length > 1
    ? first.title + ' + ' + (review.products.length - 1) + ' more'
    : first.title;
}

/* ── Table ────────────────────────────────────────────────────  */

function reviewRow(review) {
  const tr = document.createElement('tr');

  const text = document.createElement('td');
  text.style.maxWidth = '360px';
  const head = document.createElement('div');
  head.className = 'cell-strong';
  head.appendChild(stars(review.rating));
  head.appendChild(document.createTextNode(review.title || ''));
  const excerpt = document.createElement('div');
  excerpt.className = 'cell-sub';
  excerpt.style.cssText = 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap;';
  excerpt.textContent = review.body;
  text.append(head, excerpt);
  tr.appendChild(text);

  const product = document.createElement('td');
  const name = document.createElement('div');
  name.className = 'cell-strong';
  name.textContent = productLabel(review);
  product.appendChild(name);
  if (review.scope === 'Order') {
    const sub = document.createElement('div');
    sub.className = 'cell-sub';
    sub.textContent = 'Whole-order review';
    product.appendChild(sub);
  }
  tr.appendChild(product);

  const who = document.createElement('td');
  const author = document.createElement('div');
  author.className = 'cell-strong';
  author.textContent = review.author;
  const order = document.createElement('div');
  order.className = 'cell-sub';
  order.textContent = review.order_number ? '#' + review.order_number : '';
  who.append(author, order);
  tr.appendChild(who);

  const posted = cell(dateTime(review.created_at));
  posted.className = 'nowrap';
  posted.title = ago(review.created_at);
  tr.appendChild(posted);

  const status = document.createElement('td');
  status.appendChild(statusTag(review.status));
  if (review.reply) {
    const sub = document.createElement('div');
    sub.className = 'cell-sub';
    sub.textContent = 'Replied';
    status.appendChild(sub);
  }
  tr.appendChild(status);

  const actions = document.createElement('td');
  actions.className = 'col-actions';
  actions.appendChild(rowButton('Open', () => openReview(review), { title: 'Read and moderate this review' }));
  tr.appendChild(actions);

  return tr;
}

/* ── Detail ───────────────────────────────────────────────────  */

function fact(label, value) {
  const box = document.createElement('div');
  const l = document.createElement('div');
  l.className = 'cell-sub';
  l.textContent = label;
  const v = document.createElement('div');
  v.className = 'cell-strong';
  v.textContent = value;
  box.append(l, v);
  return box;
}

function openReview(review) {
  const body = document.createElement('div');

  const facts = document.createElement('div');
  facts.style.cssText = 'display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));gap:14px;margin-bottom:18px;';
  facts.append(
    fact('Customer', review.author + (review.author_email ? ' · ' + review.author_email : '')),
    fact('Order', review.order_number ? '#' + review.order_number : '—'),
    fact('Posted', dateTime(review.created_at)),
    fact('Status', review.status === 'Published' ? 'Published' : 'Hidden'),
  );
  body.appendChild(facts);

  const head = document.createElement('div');
  head.style.cssText = 'font-size:15px;font-weight:700;color:var(--dark);margin-bottom:6px;';
  head.appendChild(stars(review.rating));
  head.appendChild(document.createTextNode(review.title || ''));
  body.appendChild(head);

  const text = document.createElement('p');
  text.style.cssText = 'margin:0 0 18px;font-size:13.5px;line-height:1.65;color:var(--dark);white-space:pre-wrap;';
  text.textContent = review.body;
  body.appendChild(text);

  const shownOn = document.createElement('div');
  shownOn.style.cssText = 'font-size:12px;font-weight:700;color:var(--subtle);letter-spacing:.4px;margin-bottom:8px;';
  shownOn.textContent = review.products.length > 1
    ? 'SHOWN ON ' + review.products.length + ' PRODUCTS'
    : 'SHOWN ON';
  body.appendChild(shownOn);

  const list = document.createElement('div');
  list.style.cssText = 'display:flex;flex-direction:column;gap:6px;margin-bottom:18px;';
  review.products.forEach((p) => {
    const row = document.createElement(p.slug ? 'a' : 'div');
    if (p.slug) {
      row.href = '/product?slug=' + encodeURIComponent(p.slug);
      row.target = '_blank';
      row.rel = 'noopener';
    }
    row.style.cssText = 'display:flex;justify-content:space-between;gap:12px;padding:8px 10px;'
      + 'border:1px solid var(--border);border-radius:var(--radius-sm);font-size:13px;color:var(--dark);text-decoration:none;';
    const name = document.createElement('span');
    name.textContent = p.title + (p.variant_label ? ' — ' + p.variant_label : '');
    const helpful = document.createElement('span');
    helpful.style.color = 'var(--subtle)';
    helpful.textContent = p.helpful_count ? p.helpful_count + ' found helpful' : '';
    row.append(name, helpful);
    list.appendChild(row);
  });
  body.appendChild(list);

  if (review.status !== 'Published' && review.rejection_reason) {
    const why = document.createElement('p');
    why.style.cssText = 'margin:0 0 18px;font-size:13px;color:var(--mid);';
    why.textContent = 'Hidden because: ' + review.rejection_reason;
    body.appendChild(why);
  }

  const reply = textArea({
    value: review.reply ? review.reply.body : '',
    rows: 3,
    placeholder: 'A public reply from the shop, shown under the review',
  });
  body.appendChild(field('Shop reply', reply,
    review.reply
      ? 'Last replied by ' + review.reply.replied_by + ', ' + ago(review.reply.replied_at) + '. Clear it and save to remove.'
      : 'Optional. Shown under the review on the product page.'));

  const published = review.status === 'Published';

  openModal({
    title: review.scope === 'Order' ? 'Whole-order review' : 'Review',
    body,
    width: 640,
    actions: [
      {
        label: 'Delete',
        className: 'btn-ghost',
        onClick: async (close) => {
          const confirmed = await confirmModal({
            title: 'Delete this review?',
            message: (review.products.length > 1
              ? 'It comes off all ' + review.products.length + ' products'
              : 'It comes off the product page')
              + ' and the ratings are recalculated. The customer will be able to review again. This can’t be undone.',
            confirmLabel: 'Delete review',
            danger: true,
          });
          if (!confirmed) return;
          const response = await Fetcher.delete({ endpoint: '/v1/admin/review/' + review.uuid });
          if (!response.ok) return;
          close();
          toast.success('Review deleted');
          load();
        },
      },
      {
        label: published ? 'Hide' : 'Publish',
        className: 'btn-outline',
        onClick: async (close) => {
          if (published) {
            close();
            hideDialog(review);
            return;
          }
          const response = await Fetcher.patch({
            endpoint: '/v1/admin/review/' + review.uuid,
            body: { status: 'Published' },
          });
          if (!response.ok) return;
          close();
          toast.success('Review published again');
          load();
        },
      },
      {
        label: 'Save reply',
        className: 'btn-primary',
        onClick: async (close) => {
          const next = reply.value.trim();
          const before = review.reply ? review.reply.body : '';
          if (next === before) { close(); return; }
          const response = await Fetcher.post({
            endpoint: '/v1/admin/review/' + review.uuid + '/reply',
            body: { body: next },
          });
          if (!response.ok) return;
          close();
          toast.success(next ? 'Reply posted' : 'Reply removed');
          load();
        },
      },
    ],
  });
}

function hideDialog(review) {
  const body = document.createElement('div');
  const reason = textArea({ rows: 3, placeholder: 'Spam, abusive language, not about the product…' });
  body.appendChild(field('Reason (staff only)', reason,
    'The review comes off ' + (review.products.length > 1 ? 'all ' + review.products.length + ' products' : 'the product page')
    + ' and stops counting towards the rating. You can publish it again later.'));

  openModal({
    title: 'Hide review',
    body,
    width: 480,
    actions: [{
      label: 'Hide review',
      className: 'btn-primary',
      onClick: async (close) => {
        const response = await Fetcher.patch({
          endpoint: '/v1/admin/review/' + review.uuid,
          body: { status: 'Rejected', reason: reason.value.trim() || null },
        });
        if (!response.ok) return;
        close();
        toast.success('Review hidden');
        load();
      },
    }],
  });
}

/* ── Load ─────────────────────────────────────────────────────  */

function query(extra) {
  const q = { page: state.page, limit: state.limit };
  if (state.status !== 'all') q.status = state.status;
  if (state.rating) q.rating = state.rating;
  if (state.q) q.q = state.q;
  return Object.assign(q, extra || {});
}

async function load() {
  const tbody = $('.admin-table tbody');
  const response = await Fetcher.get({ endpoint: '/v1/admin/review/list', query: query() });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load reviews" });
    $('[data-count]').textContent = '';
    return;
  }

  const { items, total, stats } = response.data;
  fillTable(tbody, items.map(reviewRow), {
    empty: state.q || state.rating || state.status !== 'all'
      ? 'No reviews match that filter'
      : 'No reviews yet',
  });

  const from = total ? (state.page - 1) * state.limit + 1 : 0;
  const to = Math.min(state.page * state.limit, total);
  $('[data-count]').textContent = total
    ? 'Showing ' + from + '–' + to + ' of ' + total.toLocaleString() + ' reviews'
    : 'No reviews';

  const hidden = (stats && stats.hidden_rows) || 0;
  $('[data-subtitle]').textContent = 'Reviews go live as soon as a customer posts them'
    + (hidden ? ' · ' + hidden + ' hidden' : '');

  paintPager($('.pagination'), { total, limit: state.limit, page: state.page }, (page) => {
    state.page = page;
    load();
  });

  paintTabCounts();
}

/* One `limit: 1` call per tab, under the current rating and search,
   reads each total off the envelope without pulling rows. */
async function paintTabCounts() {
  const results = await Promise.all(TABS.map((status) => Fetcher.get({
    endpoint: '/v1/admin/review/list',
    query: query({ status: status === 'all' ? undefined : status, page: 1, limit: 1 }),
    showError: false,
  })));

  $$('.admin-tab').forEach((tabEl, index) => {
    const count = tabEl.querySelector('.admin-tab-count');
    if (!count) return;
    const result = results[index];
    count.textContent = result && result.ok ? (result.data.total || 0).toLocaleString() : '0';
  });
}

/* ── Boot ─────────────────────────────────────────────────────  */

(async function init() {
  const profile = await mountAdminChrome();
  if (!profile) return;

  $$('.admin-tab').forEach((tabEl) => {
    const activate = () => {
      $$('.admin-tab').forEach((t) => t.classList.remove('active'));
      tabEl.classList.add('active');
      state.status = tabEl.dataset.status || 'all';
      state.page = 1;
      load();
    };
    tabEl.addEventListener('click', activate);
    tabEl.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        activate();
      }
    });
  });

  const search = $('.admin-search input');
  if (search) {
    search.addEventListener('input', debounce(() => {
      state.q = search.value.trim();
      state.page = 1;
      load();
    }));
  }

  const rating = $('[data-rating]');
  if (rating) {
    rating.addEventListener('change', () => {
      state.rating = rating.value;
      state.page = 1;
      load();
    });
  }

  load();
})();
