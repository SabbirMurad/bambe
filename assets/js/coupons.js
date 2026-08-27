/* ═══════════════════════════════════════════════════════════════
   Bambe Web — coupons.js

   The coupon table and the Create panel below it, against the five
   `/v1/admin/coupon/*` endpoints.

   The one thing worth knowing before reading any of this: `value`
   means two different things depending on `discount_type`.
   Percentage stores **basis points** — 2000 is 20% — and Fixed
   stores **cents**. FreeShipping ignores it entirely. Every read
   and write below goes through `toValue` / `fromValue` so the
   conversion happens in exactly two places.

   Status — Active, Scheduled, Expired, Used up, Disabled — is not
   stored. It is derived from the switch, the dates and the
   redemption count, which is also how the endpoint's own tabs
   filter, so the pill and the tab can never disagree.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, fullDate, dateValue, cell, fillTable, debounce,
  rowButton, tag, mountAdminChrome, openModal, confirmModal, field, textInput,
  selectInput,
} from './admin-core.js';

const TABS = [
  { value: 'all', label: 'All' },
  { value: 'active', label: 'Active' },
  { value: 'scheduled', label: 'Scheduled' },
  { value: 'expired', label: 'Expired' },
  { value: 'disabled', label: 'Disabled' },
];

const TYPES = [
  { value: '', label: 'All types' },
  { value: 'Percentage', label: 'Percentage' },
  { value: 'Fixed', label: 'Fixed amount' },
  { value: 'FreeShipping', label: 'Free shipping' },
];

const SORTS = [
  { value: 'newest', label: 'Recently created' },
  { value: 'oldest', label: 'Oldest first' },
  { value: 'most_used', label: 'Most redeemed' },
  { value: 'value', label: 'Largest discount' },
];

const TYPE_TAG = { Percentage: 'tag-blue', Fixed: 'tag-gold', FreeShipping: 'tag-green' };
const TYPE_LABEL = { Percentage: 'Percentage', Fixed: 'Fixed amount', FreeShipping: 'Free shipping' };

let state = { tab: 'all', q: '', type: '', sort: 'newest', page: 1, limit: 50 };
let rows = [];
let tbody = null;
let counts = {};

/* ── The two meanings of `value` ──────────────────────────────  */

/** What the operator typed → what the model stores. */
function toValue(type, typed) {
  const number = Number(typed);
  if (!Number.isFinite(number)) return 0;
  if (type === 'Percentage') return Math.round(number * 100);  // 20 → 2000 bps
  if (type === 'Fixed') return Math.round(number * 100);       // 5  → 500 cents
  return 0;                                                    // FreeShipping
}

/** What the model stores → what the operator sees in a field. */
function fromValue(type, stored) {
  if (type === 'Percentage') return (stored / 100).toString();
  if (type === 'Fixed') return (stored / 100).toFixed(2);
  return '';
}

function valueLabel(coupon) {
  if (coupon.discount_type === 'Percentage') return (coupon.value / 100) + '% off';
  if (coupon.discount_type === 'Fixed') return money(coupon.value) + ' off';
  return 'Free shipping';
}

/* ── Derived status ───────────────────────────────────────────  */

function statusOf(coupon) {
  const now = Date.now();
  if (!coupon.enabled) return { key: 'disabled', label: 'Disabled', tag: 'tag-red' };
  if (coupon.total_limit && coupon.used_count >= coupon.total_limit) {
    return { key: 'used_up', label: 'Used up', tag: 'tag-red' };
  }
  if (coupon.starts_at > now) return { key: 'scheduled', label: 'Scheduled', tag: 'tag-blue' };
  if (coupon.ends_at && coupon.ends_at < now) return { key: 'expired', label: 'Expired', tag: 'tag-red' };
  return { key: 'active', label: 'Active', tag: 'tag-green' };
}

function runsNote(coupon) {
  const now = Date.now();
  const day = 24 * 60 * 60 * 1000;
  if (coupon.starts_at > now) {
    const days = Math.ceil((coupon.starts_at - now) / day);
    return 'Starts in ' + days + (days === 1 ? ' day' : ' days');
  }
  if (!coupon.ends_at) return 'No end date';
  const left = Math.ceil((coupon.ends_at - now) / day);
  if (left < 0) return 'Ended ' + Math.abs(left) + (Math.abs(left) === 1 ? ' day ago' : ' days ago');
  if (left === 0) return 'Ends today';
  return left + (left === 1 ? ' day left' : ' days left');
}

/* ── Rows ─────────────────────────────────────────────────────  */

function codeCell(coupon) {
  const wrap = document.createElement('div');

  const code = document.createElement('span');
  code.className = 'coupon-code';
  code.tabIndex = 0;
  code.setAttribute('role', 'button');
  code.title = 'Copy code';
  code.innerHTML = '<svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">'
    + '<rect x="3.5" y="3.5" width="6.5" height="6.5" rx="1.2" stroke="currentColor" stroke-width="1.2"/>'
    + '<path d="M8 3.5V2.6a1 1 0 0 0-1-1H2.6a1 1 0 0 0-1 1V7a1 1 0 0 0 1 1h.9" stroke="currentColor" stroke-width="1.2"/></svg> ';
  code.append(coupon.code);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(coupon.code);
      toast.success(coupon.code + ' copied');
    } catch (e) {
      // Clipboard access can be refused; saying so beats silence.
      toast.info('Copy it by hand: ' + coupon.code);
    }
  };
  code.addEventListener('click', copy);
  code.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); copy(); }
  });

  wrap.appendChild(code);

  const note = document.createElement('div');
  note.className = 'cell-sub';
  note.textContent = coupon.description || '';
  wrap.appendChild(note);

  if (coupon.first_order_only) {
    const badge = document.createElement('div');
    badge.className = 'cell-sub';
    badge.textContent = 'First order only';
    wrap.appendChild(badge);
  }

  return wrap;
}

function usageCell(coupon) {
  const wrap = document.createElement('div');
  wrap.className = 'coupon-usage';

  const used = document.createElement('span');
  used.className = 'num';
  used.textContent = (coupon.used_count || 0).toLocaleString();
  wrap.appendChild(used);

  const limit = document.createElement('span');
  limit.className = 'cell-sub';
  limit.style.display = 'inline';
  limit.textContent = coupon.total_limit ? ' / ' + coupon.total_limit.toLocaleString() : ' · no limit';
  wrap.appendChild(limit);

  if (coupon.total_limit) {
    const bar = document.createElement('div');
    bar.className = 'coupon-usage-bar';
    const fill = document.createElement('div');
    fill.className = 'coupon-usage-fill';
    fill.style.width = Math.min(100, Math.round((coupon.used_count / coupon.total_limit) * 100)) + '%';
    bar.appendChild(fill);
    wrap.appendChild(bar);
  }

  return wrap;
}

function enabledToggle(coupon) {
  const el = document.createElement('div');
  el.className = 'toggle' + (coupon.enabled ? ' on' : '');
  el.dataset.readonly = 'true';        // toggle.js must not also flip it
  el.setAttribute('role', 'switch');
  el.tabIndex = 0;
  el.setAttribute('aria-checked', String(coupon.enabled));
  el.setAttribute('aria-label', 'Enabled at checkout');

  const flip = async () => {
    const next = !el.classList.contains('on');
    el.classList.toggle('on', next);
    el.setAttribute('aria-checked', String(next));

    const response = await Fetcher.patch({
      endpoint: '/v1/admin/coupon/' + coupon.uuid,
      body: { enabled: next },
    });
    if (!response.ok) {
      el.classList.toggle('on', !next);
      el.setAttribute('aria-checked', String(!next));
      return;
    }

    coupon.enabled = next;
    toast.success(coupon.code + (next ? ' enabled' : ' disabled'));
    load();
    loadCounts();
  };

  el.addEventListener('click', flip);
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
  });
  return el;
}

function couponRow(coupon) {
  const tr = document.createElement('tr');
  tr.dataset.id = coupon.uuid;

  const check = document.createElement('input');
  check.type = 'checkbox';
  check.className = 'admin-check';
  check.setAttribute('data-row-check', '');
  check.setAttribute('aria-label', 'Select ' + coupon.code);
  tr.appendChild(cell(check));

  tr.appendChild(cell(codeCell(coupon)));
  tr.appendChild(cell(tag(TYPE_LABEL[coupon.discount_type] || coupon.discount_type,
    TYPE_TAG[coupon.discount_type] || 'tag-blue')));

  const value = document.createElement('div');
  const headline = document.createElement('span');
  headline.className = 'coupon-value';
  headline.textContent = valueLabel(coupon);
  value.appendChild(headline);
  const conditions = [];
  if (coupon.min_spend) conditions.push('Min spend ' + money(coupon.min_spend));
  if (coupon.max_discount) conditions.push('Up to ' + money(coupon.max_discount));
  if (conditions.length) {
    const sub = document.createElement('div');
    sub.className = 'cell-sub';
    sub.textContent = conditions.join(' · ');
    value.appendChild(sub);
  }
  tr.appendChild(cell(value));

  tr.appendChild(cell(usageCell(coupon)));

  const runs = document.createElement('div');
  runs.append(fullDate(coupon.starts_at) + ' – ' + (coupon.ends_at ? fullDate(coupon.ends_at) : 'open'));
  const note = document.createElement('div');
  note.className = 'cell-sub';
  note.textContent = runsNote(coupon);
  runs.appendChild(note);
  tr.appendChild(cell(runs));

  const status = statusOf(coupon);
  tr.appendChild(cell(tag(status.label, status.tag)));
  tr.appendChild(cell(enabledToggle(coupon)));

  const actions = document.createElement('div');
  actions.style.cssText = 'display:flex;gap:10px;justify-content:flex-end;';
  actions.appendChild(rowButton('Edit', () => edit(coupon)));
  actions.appendChild(rowButton('Duplicate', () => duplicate(coupon)));
  actions.appendChild(rowButton('Delete', () => remove(coupon), { danger: true }));
  tr.appendChild(cell(actions, 'col-actions'));

  return tr;
}

/* ── Edit ─────────────────────────────────────────────────────  */

function dateInput(ms) {
  const input = document.createElement('input');
  input.type = 'date';
  input.className = 'form-input';
  input.value = dateValue(ms);
  return input;
}

function edit(coupon) {
  const form = document.createElement('div');

  const description = textInput({ value: coupon.description || '' });
  form.appendChild(field('Description', description));

  const type = selectInput(
    TYPES.filter((t) => t.value).map(({ value, label }) => ({ value, label })),
    coupon.discount_type);
  form.appendChild(field('Discount type', type));

  const value = textInput({ value: fromValue(coupon.discount_type, coupon.value) });
  const valueField = field('Value', value);
  form.appendChild(valueField);

  const hint = document.createElement('div');
  hint.style.cssText = 'font-size:12px;color:var(--subtle);margin-top:-10px;margin-bottom:16px;';
  form.appendChild(hint);

  const refreshHint = () => {
    if (type.value === 'Percentage') {
      hint.textContent = 'Percent off the subtotal — enter 20 for 20%.';
      value.disabled = false;
    } else if (type.value === 'Fixed') {
      hint.textContent = 'Dollars off the subtotal — enter 5 for $5.00.';
      value.disabled = false;
    } else {
      hint.textContent = 'Free shipping ignores the value.';
      value.disabled = true;
    }
  };
  type.addEventListener('change', refreshHint);
  refreshHint();

  const minSpend = textInput({ value: coupon.min_spend ? (coupon.min_spend / 100).toFixed(2) : '' });
  form.appendChild(field('Minimum spend', minSpend, 'Blank means no minimum.'));

  const maxDiscount = textInput({ value: coupon.max_discount ? (coupon.max_discount / 100).toFixed(2) : '' });
  form.appendChild(field('Maximum discount', maxDiscount, 'Caps a percentage code. Blank means no cap.'));

  const totalLimit = textInput({ value: coupon.total_limit ? String(coupon.total_limit) : '' });
  form.appendChild(field('Total redemptions', totalLimit, 'Blank means unlimited.'));

  const perUser = textInput({ value: coupon.per_user_limit ? String(coupon.per_user_limit) : '' });
  form.appendChild(field('Per customer', perUser, 'Blank means unlimited.'));

  const starts = dateInput(coupon.starts_at);
  form.appendChild(field('Starts', starts));

  const ends = dateInput(coupon.ends_at);
  form.appendChild(field('Ends', ends, 'Blank runs forever.'));

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;';
  form.appendChild(error);

  openModal({
    title: 'Edit ' + coupon.code,
    body: form,
    width: '540px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (close) => close() },
      {
        label: 'Save',
        className: 'btn-primary',
        onClick: async (close, button) => {
          if (type.value !== 'FreeShipping' && !(Number(value.value) > 0)) {
            error.textContent = 'The value has to be a positive number.';
            error.style.display = '';
            return;
          }

          const cents = (input) => (input.value.trim() ? Math.round(Number(input.value) * 100) : null);
          const whole = (input) => (input.value.trim() ? Number(input.value) : null);

          button.disabled = true;
          const response = await Fetcher.patch({
            endpoint: '/v1/admin/coupon/' + coupon.uuid,
            body: {
              description: description.value.trim(),
              discount_type: type.value,
              value: toValue(type.value, value.value),
              min_spend: cents(minSpend),
              max_discount: cents(maxDiscount),
              total_limit: whole(totalLimit),
              per_user_limit: whole(perUser),
              starts_at: starts.value ? new Date(starts.value + 'T00:00:00').getTime() : coupon.starts_at,
              ends_at: ends.value ? new Date(ends.value + 'T23:59:59').getTime() : null,
            },
          });
          button.disabled = false;
          if (!response.ok) return;

          close();
          toast.success('Coupon updated');
          load();
          loadCounts();
        },
      },
    ],
  });
}

async function duplicate(coupon) {
  const response = await Fetcher.post({
    endpoint: '/v1/admin/coupon/' + coupon.uuid + '/duplicate',
    body: {},
  });
  if (!response.ok) return;
  toast.success('Duplicated as ' + (response.data.code || 'a new code'));
  load();
  loadCounts();
}

async function remove(coupon) {
  const confirmed = await confirmModal({
    title: 'Delete ' + coupon.code + '?',
    message: coupon.used_count
      ? 'It has been redeemed ' + coupon.used_count
        + (coupon.used_count === 1 ? ' time. That history stays on the orders.' : ' times. That history stays on the orders.')
      : 'It has never been redeemed.',
    confirmLabel: 'Delete coupon',
  });
  if (!confirmed) return;

  const response = await Fetcher.delete({ endpoint: '/v1/admin/coupon/' + coupon.uuid });
  if (!response.ok) return;
  toast.success('Coupon deleted');
  load();
  loadCounts();
}

/* ── Bulk ─────────────────────────────────────────────────────  */

function selectedIds() {
  return $$('[data-row-check]:checked').map((box) => box.closest('tr').dataset.id).filter(Boolean);
}

async function runOnSelection(label, run, { danger = false, message } = {}) {
  const ids = selectedIds();
  if (!ids.length) return;

  const confirmed = await confirmModal({
    title: label + ' ' + ids.length + (ids.length === 1 ? ' coupon?' : ' coupons?'),
    message: message || 'One request per coupon — there is no bulk endpoint behind it.',
    confirmLabel: label,
    danger,
  });
  if (!confirmed) return;

  let failed = 0;
  for (const id of ids) {
    const response = await run(id);
    if (!response.ok) failed++;
  }

  if (failed) toast.error(failed + ' of ' + ids.length + ' failed');
  else toast.success('Done');
  load();
  loadCounts();
}

/* ── Toolbar ──────────────────────────────────────────────────  */

function paintTabs() {
  const host = $('.admin-tabs');
  if (!host) return;
  host.innerHTML = '';

  TABS.forEach(({ value, label }) => {
    const el = document.createElement('div');
    el.className = 'admin-tab' + (value === state.tab ? ' active' : '');
    el.setAttribute('role', 'tab');
    el.tabIndex = 0;
    el.textContent = label;

    if (counts[value] !== undefined) {
      const count = document.createElement('span');
      count.className = 'admin-tab-count';
      count.textContent = String(counts[value]);
      el.appendChild(count);
    }

    const pick = () => { state.tab = value; load(); };
    el.addEventListener('click', pick);
    el.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(); }
    });
    host.appendChild(el);
  });
}

async function loadCounts() {
  const responses = await Promise.all(TABS.map(({ value }) =>
    Fetcher.get({
      endpoint: '/v1/admin/coupon/list',
      query: { tab: value, limit: 1 },
      showError: false,
    })));

  TABS.forEach(({ value }, index) => {
    counts[value] = responses[index].ok ? responses[index].data.total : 0;
  });
  paintTabs();
}

function exportCodes() {
  if (!rows.length) return;
  const header = ['Code', 'Description', 'Type', 'Value', 'Min spend', 'Used', 'Limit', 'Starts', 'Ends', 'Enabled'];
  const lines = [header, ...rows.map((coupon) => [
    coupon.code,
    coupon.description,
    coupon.discount_type,
    valueLabel(coupon),
    coupon.min_spend ? (coupon.min_spend / 100).toFixed(2) : '',
    coupon.used_count,
    coupon.total_limit || '',
    dateValue(coupon.starts_at),
    dateValue(coupon.ends_at),
    coupon.enabled ? 'yes' : 'no',
  ])];

  const csv = lines
    .map((row) => row.map((value) => '"' + String(value).replace(/"/g, '""') + '"').join(','))
    .join('\n');

  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'bambe-coupons.csv';
  link.click();
  URL.revokeObjectURL(url);
  toast.success('Saved as CSV');
}

/* ── The Create panel ─────────────────────────────────────────  */

function wireCreatePanel() {
  const code = $('#cp-code');
  const type = $('#cp-type');
  const value = $('#cp-value');
  const valueHint = $('#cp-value-hint');
  const min = $('#cp-min');
  const limit = $('#cp-limit');
  const note = $('#cp-note');
  const starts = $('#cp-start');
  const ends = $('#cp-end');
  const enabled = $('#cp-enabled');
  const create = $('#cp-create');
  const message = $('#cp-msg');
  if (!create) return;

  /* The wireframe's select used percent/fixed/ship. The model's
     variants are what actually go on the wire. */
  if (type) {
    type.innerHTML = '';
    TYPES.filter((t) => t.value).forEach(({ value: v, label }) => {
      const option = document.createElement('option');
      option.value = v;
      option.textContent = label;
      type.appendChild(option);
    });
  }

  const refreshHint = () => {
    if (!valueHint || !type) return;
    if (type.value === 'Percentage') {
      valueHint.textContent = 'Percent off the order subtotal — 20 means 20%.';
      value.disabled = false;
    } else if (type.value === 'Fixed') {
      valueHint.textContent = 'Dollars off the order subtotal — 5 means $5.00.';
      value.disabled = false;
    } else {
      valueHint.textContent = 'Free shipping ignores the value.';
      value.disabled = true;
    }
  };
  if (type) type.addEventListener('change', refreshHint);
  refreshHint();

  if (code) {
    code.addEventListener('input', () => {
      code.value = code.value.toUpperCase().replace(/[^A-Z0-9]/g, '');
    });
  }

  if (enabled) {
    enabled.dataset.readonly = 'true';
    enabled.setAttribute('role', 'switch');
    enabled.setAttribute('aria-checked', 'true');
    enabled.tabIndex = 0;
    const flip = () => {
      const on = !enabled.classList.contains('on');
      enabled.classList.toggle('on', on);
      enabled.setAttribute('aria-checked', String(on));
    };
    enabled.addEventListener('click', flip);
    enabled.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(); }
    });
  }

  // Default the start date to today, so a code created now is live now.
  if (starts && !starts.value) starts.value = dateValue(Date.now());

  const say = (text, bad) => {
    if (!message) return;
    message.style.display = text ? '' : 'none';
    message.style.color = bad ? 'var(--error)' : 'var(--subtle)';
    message.textContent = text;
  };

  create.addEventListener('click', async () => {
    if (!code.value.trim()) return say('Give the coupon a code.', true);
    if (type.value !== 'FreeShipping' && !(Number(value.value) > 0)) {
      return say('The value has to be a positive number.', true);
    }

    create.disabled = true;
    const response = await Fetcher.post({
      endpoint: '/v1/admin/coupon',
      body: {
        code: code.value.trim().toUpperCase(),
        description: note && note.value.trim() ? note.value.trim() : code.value.trim(),
        discount_type: type.value,
        value: toValue(type.value, value.value),
        min_spend: min && Number(min.value) > 0 ? Math.round(Number(min.value) * 100) : null,
        max_discount: null,
        total_limit: limit && Number(limit.value) > 0 ? Number(limit.value) : null,
        per_user_limit: null,
        starts_at: starts && starts.value
          ? new Date(starts.value + 'T00:00:00').getTime()
          : Date.now(),
        ends_at: ends && ends.value ? new Date(ends.value + 'T23:59:59').getTime() : null,
        enabled: enabled ? enabled.classList.contains('on') : true,
        first_order_only: false,
      },
    });
    create.disabled = false;
    if (!response.ok) return;

    code.value = '';
    if (value) value.value = '';
    if (note) note.value = '';
    say('Created. Edit it in the table above to set a per-customer limit or a cap.');
    toast.success('Coupon created');
    load();
    loadCounts();
  });
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({
    endpoint: '/v1/admin/coupon/list',
    query: { tab: state.tab, type: state.type, sort: state.sort, page: state.page, limit: state.limit },
  });

  if (!response.ok) {
    fillTable(tbody, [], { empty: "Couldn't load coupons" });
    return;
  }

  /* The endpoint has no `q`, so the search box filters the page it
     already has. With a coupon table that is the whole set anyway —
     the limit is 50 and nobody runs 50 codes at once — but the
     placeholder says "on this page" so it cannot mislead. */
  const needle = state.q.toLowerCase();
  rows = (response.data.items || []).filter((coupon) => !needle
    || coupon.code.toLowerCase().includes(needle)
    || (coupon.description || '').toLowerCase().includes(needle));

  fillTable(tbody, rows.map(couponRow), {
    empty: state.q ? 'No coupons match “' + state.q + '”' : 'Nothing in this tab',
  });

  const foot = $('#cp-foot');
  if (foot) {
    foot.textContent = 'Showing ' + rows.length + ' of ' + response.data.total
      + (response.data.total === 1 ? ' coupon' : ' coupons');
  }

  const subtitle = $('#cp-subtitle');
  if (subtitle) {
    const live = rows.filter((c) => statusOf(c).key === 'active').length;
    const redeemed = rows.reduce((sum, c) => sum + (c.used_count || 0), 0);
    subtitle.textContent = live + ' running now · ' + redeemed.toLocaleString() + ' redemptions';
  }

  const all = $('[data-check-all]');
  if (all) { all.checked = false; all.indeterminate = false; }
  const bulkBar = $('[data-bulk]');
  if (bulkBar) bulkBar.classList.remove('visible');
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  tbody = $('#cp-body');
  if (!tbody) return;

  const [typeSelect, sortSelect] = $$('.admin-toolbar select.admin-filter');

  const fillSelect = (select, options, onChange) => {
    if (!select) return;
    select.innerHTML = '';
    options.forEach(({ value, label }) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      select.appendChild(option);
    });
    select.addEventListener('change', () => { onChange(select.value); load(); });
  };

  fillSelect(typeSelect, TYPES, (value) => { state.type = value; });
  fillSelect(sortSelect, SORTS, (value) => { state.sort = value; });

  const search = $('.admin-search input');
  if (search) {
    search.placeholder = 'Filter this page by code or description…';
    search.addEventListener('input', debounce(() => {
      state.q = search.value.trim();
      load();
    }, 200));
  }

  const exportButton = $('#cp-export');
  if (exportButton) {
    exportButton.textContent = 'Export CSV';
    exportButton.addEventListener('click', exportCodes);
  }

  const jump = $('#cp-jump-new');
  if (jump) {
    jump.addEventListener('click', () => {
      const panel = $('#cp-new');
      if (panel) panel.scrollIntoView({ behavior: 'smooth', block: 'start' });
      const code = $('#cp-code');
      if (code) setTimeout(() => code.focus(), 400);
    });
  }

  const [enable, disable, dup, del] = $$('[data-bulk] button');
  if (enable) {
    enable.addEventListener('click', () => runOnSelection('Enable', (id) =>
      Fetcher.patch({ endpoint: '/v1/admin/coupon/' + id, body: { enabled: true }, showError: false })));
  }
  if (disable) {
    disable.addEventListener('click', () => runOnSelection('Disable', (id) =>
      Fetcher.patch({ endpoint: '/v1/admin/coupon/' + id, body: { enabled: false }, showError: false })));
  }
  if (dup) {
    dup.addEventListener('click', () => runOnSelection('Duplicate', (id) =>
      Fetcher.post({ endpoint: '/v1/admin/coupon/' + id + '/duplicate', body: {}, showError: false })));
  }
  if (del) {
    del.addEventListener('click', () => runOnSelection('Delete', (id) =>
      Fetcher.delete({ endpoint: '/v1/admin/coupon/' + id, showError: false }),
    { danger: true, message: 'Redemption history stays on the orders that used them.' }));
  }

  wireCreatePanel();
  loadCounts();
  load();
})();
