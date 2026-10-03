/* ═══════════════════════════════════════════════════════════════
   Bambe Web — admin-returns.js

   The staff side of returns, against the three
   `/v1/admin/return/*` endpoints.

   Returns were previously a dead end: a customer could file one
   from `/account/returns` and nothing could ever act on it — there
   was no endpoint to approve, reject or refund, so every request
   sat at "Requested" for good. `refunded_quantity` on the order
   line was read in three places and written in none, which is why
   the same line could be returned again and again.

   Resolving to **Refunded** is the terminal step and the only one
   that moves money: it marks the order lines, restocks (unless
   staff say the goods came back damaged), writes the refund
   transaction and mails the customer. It is confirmed rather than
   done on a click, and the endpoint refuses a second attempt.

   A cash-on-delivery order that was never collected shows a
   refund of zero — there is nothing to give back, but the lines
   are still marked and the stock still returns. The dialog says
   so rather than pretending money moved.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, dateTime, ago, spaced, debounce, tag,
  cell, stacked, rowButton, fillTable, paintPager, mountAdminChrome,
  openModal, confirmModal, field, textArea, selectInput,
} from './admin-core.js';

const TABS = [
  { value: 'all', label: 'All' },
  { value: 'requested', label: 'Requested' },
  { value: 'approved', label: 'Approved' },
  { value: 'in_transit', label: 'In transit' },
  { value: 'received', label: 'Received' },
  { value: 'refunded', label: 'Refunded' },
  { value: 'rejected', label: 'Rejected' },
];

/* Mirrors ReturnStatus in model/order.rs. Refunded is green because
   it is the resolved-happily end state; Rejected is red. */
const STATUS_TAG = {
  Requested: 'tag-gold',
  Approved: 'tag-blue',
  InTransit: 'tag-blue',
  Received: 'tag-blue',
  Refunded: 'tag-green',
  Rejected: 'tag-red',
};

/* What a return can move to from where it is. `Refunded` and
   `Rejected` are terminal — the endpoint enforces this too, but a
   dropdown that offers an option the server refuses is worse than
   one that doesn't. */
const NEXT = {
  Requested: ['Approved', 'Rejected'],
  Approved: ['InTransit', 'Received', 'Refunded', 'Rejected'],
  InTransit: ['Received', 'Refunded'],
  Received: ['Refunded', 'Rejected'],
  Refunded: [],
  Rejected: [],
};

let state = { status: 'all', q: '', page: 1, limit: 20 };
let rows = [];

function statusTag(status) {
  return tag(spaced(status), STATUS_TAG[status] || 'tag-blue');
}

/* ── Table ────────────────────────────────────────────────────  */

function returnRow(item) {
  const tr = document.createElement('tr');

  tr.appendChild(cell(item.order_number ? '#' + item.order_number : '—', 'cell-id'));
  tr.appendChild(stacked(item.customer || 'Unknown', null));
  tr.appendChild(cell(item.reason || '—'));

  const requested = cell(dateTime(item.requested_at));
  requested.className = 'nowrap';
  requested.title = ago(item.requested_at);
  tr.appendChild(requested);

  const units = cell(String(item.unit_count ?? 0));
  units.className = 'num';
  tr.appendChild(units);

  const status = document.createElement('td');
  status.appendChild(statusTag(item.status));
  if (item.status === 'Refunded' && typeof item.refund_amount === 'number') {
    const sub = document.createElement('div');
    sub.className = 'cell-sub';
    sub.textContent = money(item.refund_amount);
    status.appendChild(sub);
  }
  tr.appendChild(status);

  const actions = document.createElement('td');
  actions.className = 'col-actions';
  actions.appendChild(rowButton('Open', () => openReturn(item.uuid), { title: 'Review this return' }));
  tr.appendChild(actions);

  return tr;
}

/* ── Detail + resolve ─────────────────────────────────────────  */

function lineRow(line) {
  const tr = document.createElement('tr');
  tr.appendChild(stacked(line.title || line.variant_id, line.variant_label || line.sku || null));

  const qty = cell(String(line.quantity));
  qty.className = 'num';
  tr.appendChild(qty);

  const price = cell(money(line.unit_price));
  price.className = 'num';
  tr.appendChild(price);

  const total = cell(money(line.line_refund));
  total.className = 'num';
  tr.appendChild(total);

  /* The line may already be part-refunded by an earlier return on
     the same order — worth showing, because it caps what this one
     can give back. */
  const already = cell(line.already_refunded ? String(line.already_refunded) + ' refunded' : '—');
  already.className = 'num';
  tr.appendChild(already);

  return tr;
}

async function openReturn(id) {
  const response = await Fetcher.get({ endpoint: '/v1/admin/return/' + id });
  if (!response.ok) return;

  const { request, order, items, refundable } = response.data;

  const body = document.createElement('div');

  const summary = document.createElement('div');
  summary.className = 'admin-detail-grid';
  summary.style.cssText = 'display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:14px;margin-bottom:18px;';
  const facts = [
    ['Order', order ? '#' + order.number : '—'],
    ['Customer', order ? order.customer : '—'],
    ['Status', spaced(request.status)],
    ['Payment', order ? spaced(order.payment_status) : '—'],
    ['Requested', dateTime(request.requested_at)],
    ['Refundable', money(refundable)],
  ];
  facts.forEach(([label, value]) => {
    const box = document.createElement('div');
    const l = document.createElement('div');
    l.className = 'cell-sub';
    l.textContent = label;
    const v = document.createElement('div');
    v.className = 'cell-strong';
    v.textContent = value;
    box.append(l, v);
    summary.appendChild(box);
  });
  body.appendChild(summary);

  const reason = document.createElement('p');
  reason.style.cssText = 'margin:0 0 18px;font-size:13px;line-height:1.6;color:var(--dark);';
  reason.textContent = request.details
    ? request.reason + ' — ' + request.details
    : request.reason;
  body.appendChild(reason);

  const scroll = document.createElement('div');
  scroll.className = 'table-scroll';
  const table = document.createElement('table');
  table.className = 'admin-table';
  table.innerHTML =
    '<thead><tr><th>Item</th><th class="num">Qty</th><th class="num">Unit</th>'
    + '<th class="num">Refund</th><th class="num">Already</th></tr></thead>';
  const tbody = document.createElement('tbody');
  items.forEach((line) => tbody.appendChild(lineRow(line)));
  table.appendChild(tbody);
  scroll.appendChild(table);
  body.appendChild(scroll);

  if (request.staff_note) {
    const note = document.createElement('p');
    note.style.cssText = 'margin:16px 0 0;font-size:13px;color:var(--mid);';
    note.textContent = 'Note: ' + request.staff_note;
    body.appendChild(note);
  }

  const options = NEXT[request.status] || [];
  const actions = [];

  if (options.length) {
    actions.push({
      label: 'Update status',
      className: 'btn-primary',
      onClick: (close) => { close(); resolveDialog(request, order, response.data, options); },
    });
  } else {
    const done = document.createElement('p');
    done.style.cssText = 'margin:16px 0 0;font-size:13px;color:var(--subtle);';
    done.textContent = request.status === 'Refunded'
      ? 'This return has been refunded. It can’t be changed again.'
      : 'This return was rejected and can’t be reopened.';
    body.appendChild(done);
  }

  openModal({
    title: 'Return on order ' + (order ? '#' + order.number : ''),
    body,
    actions,
    width: 720,
  });
}

function resolveDialog(request, order, detail, options) {
  const { refundable, line_value: lineValue, captured } = detail;
  const body = document.createElement('div');

  const status = selectInput(options.map((value) => ({ value, label: spaced(value) })), options[0]);
  body.appendChild(field('New status', status));

  const note = textArea({
    value: request.staff_note || '',
    rows: 3,
    placeholder: 'Shown to the customer in their return update email',
  });
  body.appendChild(field('Note (optional)', note));

  /* Only meaningful for the Refunded step; hidden otherwise so the
     dialog doesn't ask about money on a status move. */
  const refundWrap = document.createElement('div');

  const amount = document.createElement('input');
  amount.type = 'number';
  amount.className = 'form-input';
  amount.step = '0.01';
  amount.min = '0';
  amount.value = (refundable / 100).toFixed(2);
  refundWrap.appendChild(field('Refund amount', amount));

  const restockRow = document.createElement('label');
  restockRow.style.cssText = 'display:flex;align-items:center;gap:8px;font-size:13px;color:var(--dark);margin-top:4px;';
  const restock = document.createElement('input');
  restock.type = 'checkbox';
  restock.className = 'admin-check';
  restock.checked = true;
  restockRow.append(restock, document.createTextNode('Put these units back on sale'));
  refundWrap.appendChild(restockRow);

  const capped = document.createElement('p');
  capped.style.cssText = 'margin:10px 0 0;font-size:12px;color:var(--subtle);line-height:1.5;';
  /* `refundable` is already the server's own ceiling — the line value
     capped by what was captured less what has been refunded. When it
     is short of the line value, say why, rather than offering an
     amount `resolve.rs` will silently clamp away. */
  if (refundable <= 0) {
    capped.textContent = captured > 0
      ? 'This order has already been fully refunded, so no further money is returned — the lines are still marked returned and the stock goes back.'
      : 'No payment was ever captured on this order, so no money is returned — the lines are still marked returned and the stock goes back.';
  } else if (refundable < lineValue) {
    capped.textContent = 'These lines are worth ' + money(lineValue)
      + ', but only ' + money(refundable) + ' is left of the captured payment.';
  } else {
    capped.textContent = 'Capped at what was actually charged, less anything already refunded.';
  }
  refundWrap.appendChild(capped);

  body.appendChild(refundWrap);

  const syncRefund = () => {
    refundWrap.hidden = status.value !== 'Refunded';
  };
  status.addEventListener('change', syncRefund);
  syncRefund();

  openModal({
    title: 'Update return',
    body,
    width: 520,
    actions: [{
      label: 'Save',
      className: 'btn-primary',
      onClick: async (close) => {
        const next = status.value;
        const payload = { status: next };
        if (note.value.trim()) payload.staff_note = note.value.trim();

        if (next === 'Refunded') {
          payload.refund_amount = Math.round(parseFloat(amount.value || '0') * 100);
          payload.restock = restock.checked;

          const confirmed = await confirmModal({
            title: 'Refund this return?',
            message: payload.refund_amount > 0
              ? 'This refunds ' + money(payload.refund_amount) + ' and can’t be undone.'
              : 'This marks the lines returned and restocks them. No money is returned.',
            confirmLabel: 'Refund',
            danger: true,
          });
          if (!confirmed) return;
        }

        const response = await Fetcher.patch({
          endpoint: '/v1/admin/return/' + request.uuid,
          body: payload,
        });
        if (!response.ok) return;

        close();
        toast.success(next === 'Refunded' ? 'Return refunded' : 'Return updated');
        load();
      },
    }],
  });
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const tbody = $('.admin-table tbody');
  const query = { page: state.page, limit: state.limit };
  if (state.status !== 'all') query.status = state.status;
  if (state.q) query.q = state.q;

  const response = await Fetcher.get({ endpoint: '/v1/admin/return/list', query });
  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load returns" });
    $('[data-count]').textContent = '';
    return;
  }

  const { items, total, stats } = response.data;
  rows = items;

  fillTable(tbody, items.map(returnRow), {
    empty: state.q || state.status !== 'all'
      ? 'No returns match that filter'
      : 'No return requests yet',
  });

  const from = total ? (state.page - 1) * state.limit + 1 : 0;
  const to = Math.min(state.page * state.limit, total);
  $('[data-count]').textContent = total
    ? 'Showing ' + from + '–' + to + ' of ' + total.toLocaleString() + ' returns'
    : 'No returns';

  const open = (stats && stats.open) || 0;
  $('[data-subtitle]').textContent = open
    ? open + ' awaiting action'
    : 'Nothing waiting on you';

  paintPager($('.pagination'), { total, limit: state.limit, page: state.page }, (page) => {
    state.page = page;
    load();
  });

  paintTabCounts();
}

/* Each tab shows how many sit in that state. One `limit: 1` call
   per tab reads the total off the envelope without pulling rows. */
async function paintTabCounts() {
  const results = await Promise.all(TABS.map((t) => Fetcher.get({
    endpoint: '/v1/admin/return/list',
    query: t.value === 'all' ? { limit: 1 } : { status: t.value, limit: 1 },
    showError: false,
  })));

  $$('.admin-tab').forEach((tabEl, index) => {
    const count = tabEl.querySelector('.admin-tab-count');
    if (!count) return;
    const result = results[index];
    count.textContent = result && result.ok ? (result.data.total || 0).toLocaleString() : '0';
  });
}

function exportCsv() {
  if (!rows.length) {
    toast.error('Nothing to export');
    return;
  }

  const header = ['Order', 'Customer', 'Reason', 'Units', 'Status', 'Refund', 'Requested'];
  const escape = (value) => '"' + String(value ?? '').replace(/"/g, '""') + '"';
  const lines = [header.join(',')];

  rows.forEach((r) => lines.push([
    r.order_number || '',
    r.customer || '',
    r.reason || '',
    r.unit_count ?? 0,
    r.status,
    typeof r.refund_amount === 'number' ? (r.refund_amount / 100).toFixed(2) : '',
    new Date(r.requested_at).toISOString(),
  ].map(escape).join(',')));

  const blob = new Blob([lines.join('\n')], { type: 'text/csv;charset=utf-8' });
  const link = document.createElement('a');
  link.href = URL.createObjectURL(blob);
  link.download = 'returns-page-' + state.page + '.csv';
  link.click();
  URL.revokeObjectURL(link.href);
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

  const exportBtn = $('[data-export]');
  if (exportBtn) exportBtn.addEventListener('click', exportCsv);

  load();
})();
