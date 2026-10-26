/* ═══════════════════════════════════════════════════════════════
   Bambee Web — admin-overview.js

   `GET /v1/admin/dashboard` answers the whole screen in one call:
   four headline numbers, a revenue series, the last ten orders and
   the ten variants closest to running out.

   Two things the wireframe drew that the endpoint cannot support,
   and what happened to them:

   - **"↑ 12.4% vs last period".** Nothing returns a previous
     period, so every delta was invented. Rather than compute a
     comparison the server never made, the tiles say what the
     period is instead.
   - **"Export report".** There is no export endpoint. The button
     downloads what is on screen as CSV, built in the browser from
     the same response — which is honest about being a snapshot of
     this view rather than a report the server produced.

   The chart is redrawn from `revenue_series` rather than being
   left as the hand-drawn path: same viewBox, same grid, so the
   stylesheet is untouched.
═══════════════════════════════════════════════════════════════ */
import {
  Fetcher, toast, $, $$, money, shortMoney, ago, fullDate,
  cell, stacked, orderTag, fillTable, mountAdminChrome,
} from './admin-core.js';

const PERIODS = [
  { value: '7d', label: 'Last 7 days' },
  { value: '30d', label: 'Last 30 days' },
  { value: '90d', label: 'This quarter' },
  { value: 'ytd', label: 'This year' },
];

let data = null;
let period = '30d';

/* ── Tiles ────────────────────────────────────────────────────  */

function paintTiles() {
  const tiles = $$('.stat-tile');
  const label = (PERIODS.find((p) => p.value === period) || {}).label || 'this period';

  const values = [
    money(data.revenue),
    (data.orders || 0).toLocaleString(),
    money(data.average_order_value),
    // Already a percentage from the handler, not a fraction.
    (data.refund_rate || 0).toFixed(1) + '%',
  ];

  tiles.forEach((tile, index) => {
    const value = tile.querySelector('.stat-tile-value');
    if (value) value.textContent = values[index] !== undefined ? values[index] : '—';

    /* The delta had no source. The footer says which window the
       number covers, which is the thing an operator actually needs
       to know before reading it. */
    const foot = tile.querySelector('.stat-tile-foot');
    if (foot) {
      foot.innerHTML = '';
      foot.textContent = label.toLowerCase();
    }
  });
}

/* ── Chart ────────────────────────────────────────────────────
   Same 640×210 viewBox the wireframe used, so `.chart-svg` and
   its grid lines keep working; only the path data and the labels
   are computed.                                                 */
function paintChart() {
  const svg = $('.chart-svg');
  if (!svg) return;

  const series = data.revenue_series || [];
  const line = svg.querySelector('.chart-line');
  const area = svg.querySelector('.chart-area');
  const dot = svg.querySelector('.chart-dot');

  const legendPeak = $$('.chart-legend-item')[1];

  if (series.length < 2) {
    if (line) line.setAttribute('d', '');
    if (area) area.setAttribute('d', '');
    if (dot) dot.style.display = 'none';
    if (legendPeak) legendPeak.textContent = 'Not enough data yet';
    svg.setAttribute('aria-label', 'Revenue chart — not enough data for this period yet.');
    return;
  }

  const LEFT = 44;
  const RIGHT = 620;
  const TOP = 10;
  const BOTTOM = 170;

  const peak = Math.max(...series.map((point) => point.revenue), 1);
  const x = (index) => LEFT + (index * (RIGHT - LEFT)) / (series.length - 1);
  const y = (value) => BOTTOM - (value / peak) * (BOTTOM - TOP);

  const points = series.map((point, index) => x(index).toFixed(1) + ',' + y(point.revenue).toFixed(1));
  if (line) line.setAttribute('d', 'M' + points.join(' L'));
  if (area) {
    area.setAttribute('d', 'M' + points.join(' L')
      + ' L' + RIGHT + ',' + BOTTOM + ' L' + LEFT + ',' + BOTTOM + ' Z');
  }

  const peakIndex = series.reduce((best, point, index) =>
    point.revenue > series[best].revenue ? index : best, 0);
  if (dot) {
    dot.style.display = '';
    dot.setAttribute('cx', x(peakIndex).toFixed(1));
    dot.setAttribute('cy', y(series[peakIndex].revenue).toFixed(1));
  }

  // Y axis: five ticks from 0 to the peak.
  const axis = $$('.chart-axis-label', svg).filter((label) => label.getAttribute('text-anchor') === 'end');
  axis.forEach((label, index) => {
    label.textContent = shortMoney(peak * (1 - index / (axis.length - 1)));
  });

  // X axis: the wireframe drew six month labels, evenly spaced.
  const xLabels = $$('.chart-axis-label', svg).filter((label) => label.getAttribute('text-anchor') === 'middle');
  xLabels.forEach((label, index) => {
    const at = Math.round((index * (series.length - 1)) / (xLabels.length - 1));
    const point = series[at];
    label.setAttribute('x', x(at).toFixed(1));
    label.textContent = point
      ? new Date(point.date).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
      : '';
  });

  if (legendPeak) {
    legendPeak.innerHTML = '';
    legendPeak.append('Peak — ');
    const strong = document.createElement('strong');
    strong.style.color = 'var(--black)';
    strong.textContent = money(series[peakIndex].revenue) + ' on '
      + fullDate(new Date(series[peakIndex].date).getTime());
    legendPeak.appendChild(strong);
  }

  svg.setAttribute('aria-label',
    'Revenue per day over the selected period, peaking at ' + money(series[peakIndex].revenue) + '.');
}

/* ── Panels ───────────────────────────────────────────────────  */

function paintRecentOrders() {
  // Not a placeholder copy: skeleton.js puts one beside each tbody.
  const tbody = $$('.admin-panel .admin-table tbody:not(.skeleton-clone)')[0];
  if (!tbody) return;

  const rows = (data.recent_orders || []).map((order) => {
    const tr = document.createElement('tr');
    tr.style.cursor = 'pointer';
    tr.addEventListener('click', () => {
      location.href = '/admin/orders?q=' + encodeURIComponent(order.number);
    });

    const id = document.createElement('div');
    const number = document.createElement('span');
    number.className = 'cell-id';
    number.textContent = '#' + order.number;
    const sub = document.createElement('div');
    sub.className = 'cell-sub';
    sub.textContent = ago(order.placed_at);
    id.appendChild(number);
    id.appendChild(sub);

    tr.appendChild(cell(id));
    /* The endpoint returns no customer name on this rollup, and a
       lookup per row for a five-row panel is not worth it — the
       payment state is the more useful second column anyway. */
    tr.appendChild(cell(order.payment_status || '—'));
    tr.appendChild(cell(orderTag(order.fulfilment_status)));
    tr.appendChild(cell(money(order.grand_total), 'num'));
    return tr;
  });

  fillTable(tbody, rows, { empty: 'No orders yet' });
}

function paintLowStock() {
  const tbody = $$('.admin-panel .admin-table tbody:not(.skeleton-clone)')[1];
  if (!tbody) return;

  const rows = (data.low_stock || []).map((variant) => {
    const tr = document.createElement('tr');

    const wrap = document.createElement('div');
    const name = document.createElement('div');
    name.className = 'cell-strong';
    name.textContent = variant.product_title || 'Untitled product';
    wrap.appendChild(name);

    const spec = variant.variant_label || '';
    if (spec) {
      const sub = document.createElement('div');
      sub.className = 'cell-sub';
      sub.textContent = spec;
      wrap.appendChild(sub);
    }

    /* The bar is stock against the threshold that flagged it, which
       is the only ceiling the row actually knows. */
    const threshold = Math.max(1, variant.low_stock_threshold || 1);
    const share = Math.min(100, Math.round((variant.stock / threshold) * 100));
    const bar = document.createElement('div');
    bar.className = 'stock-bar';
    const fill = document.createElement('div');
    fill.className = 'stock-bar-fill ' + (share <= 25 ? 'out' : 'low');
    fill.style.width = Math.max(4, share) + '%';
    bar.appendChild(fill);
    wrap.appendChild(bar);

    tr.appendChild(cell(wrap));
    const left = cell(String(variant.stock), 'num');
    left.style.color = share <= 25 ? 'var(--error)' : 'var(--warn)';
    tr.appendChild(left);
    return tr;
  });

  fillTable(tbody, rows, { empty: 'Nothing running low' });
}

/* ── Export ───────────────────────────────────────────────────
   No export endpoint exists, so this is the visible data, in the
   browser, saved as CSV. Named for what it is.                 */
function exportCsv() {
  if (!data) return;

  const lines = [
    ['Bambee — overview', PERIODS.find((p) => p.value === period).label],
    [],
    ['Revenue', (data.revenue / 100).toFixed(2)],
    ['Orders', data.orders],
    ['Average order value', (data.average_order_value / 100).toFixed(2)],
    ['Refund rate', (data.refund_rate || 0).toFixed(2) + '%'],
    [],
    ['Date', 'Revenue'],
    ...(data.revenue_series || []).map((point) => [point.date, (point.revenue / 100).toFixed(2)]),
  ];

  const csv = lines
    .map((row) => row.map((value) => '"' + String(value === undefined ? '' : value).replace(/"/g, '""') + '"').join(','))
    .join('\n');

  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'bambee-overview-' + period + '.csv';
  link.click();
  URL.revokeObjectURL(url);
  toast.success('Saved as CSV');
}

/* ── Load ─────────────────────────────────────────────────────  */

async function load() {
  const response = await Fetcher.get({
    endpoint: '/v1/admin/dashboard',
    query: { period },
  });
  if (!response.ok) {
    /* Every placeholder on this page wraps sample markup (tiles, a
       drawn chart, table rows), so each is given a failed state before
       its placeholder ends — clearing alone would reveal the samples. */
    $$('.stat-tile').forEach((tile) => {
      const value = tile.querySelector('.stat-tile-value');
      if (value) value.textContent = '—';
      const foot = tile.querySelector('.stat-tile-foot');
      if (foot) foot.textContent = "Couldn't load";
    });
    const chart = $('.chart-wrap');
    if (chart) {
      chart.innerHTML = '<div style="padding:48px 16px;text-align:center;color:var(--subtle);font-size:13px;">'
        + "Couldn't load revenue</div>";
    }
    const peak = $$('.chart-legend-item')[1];
    if (peak) peak.style.display = 'none';
    $$('.admin-table tbody:not(.skeleton-clone)').forEach((tbody) => fillTable(tbody, [], { empty: "Couldn't load" }));
    if (window.Skeleton) $$('[data-skeleton]').forEach((el) => window.Skeleton.clear(el));
    return;
  }

  data = response.data;

  const subtitle = $('.admin-subtitle');
  if (subtitle) {
    subtitle.textContent = 'Store performance — '
      + (PERIODS.find((p) => p.value === period) || {}).label.toLowerCase();
  }

  if (window.Skeleton) {
    $$('[data-skeleton]').forEach((el) => window.Skeleton.clear(el));
  }

  paintTiles();
  paintChart();
  paintRecentOrders();
  paintLowStock();
}

/* ── Boot ─────────────────────────────────────────────────────  */
(async function init() {
  const admin = await mountAdminChrome();
  if (!admin) return;

  // `select.` matters: select.js puts `.admin-filter` on its skin button too.
  const select = $('select.admin-filter');
  if (select) {
    select.innerHTML = '';
    PERIODS.forEach(({ value, label }) => {
      const option = document.createElement('option');
      option.value = value;
      option.textContent = label;
      if (value === period) option.selected = true;
      select.appendChild(option);
    });
    select.addEventListener('change', () => { period = select.value; load(); });
  }

  const exportButton = $('.admin-head-actions .btn-primary');
  if (exportButton) {
    exportButton.textContent = 'Export CSV';
    exportButton.addEventListener('click', exportCsv);
  }

  /* "View breakdown" had nowhere to go — there is no breakdown
     endpoint — so it points at the order list, which is the closest
     thing to one that exists. */
  const breakdown = $('.admin-panel-link[tabindex]');
  if (breakdown) {
    breakdown.textContent = 'All orders';
    breakdown.addEventListener('click', () => { location.href = '/admin/orders'; });
  }

  load();
})();
