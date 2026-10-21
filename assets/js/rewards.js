/* ═══════════════════════════════════════════════════════════════
   Bambee Web — rewards.js

   The stamp card. Every delivered order stamps it once (the server
   does that when staff mark the order Delivered); ten stamps fill
   it, and a full card is traded for a reward the shop picks for
   this customer.

   Stamps the customer hasn't seen yet press onto the card one after
   another, and the page then tells the server how far it got
   (`seen_stamps`), so the animation plays once, not on every visit.

   A full card shows one of two things: the reward with a Redeem
   button, or — when the shop hasn't set it yet — "stay tuned". The
   code itself only exists on the page after redeeming.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { mountAccountChrome } from './account.js';

const host = document.querySelector('[data-rewards]');
const STAMP_GAP_MS = 260;
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

const GIFT = '<svg viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M3 8.5h14v8.5H3V8.5ZM2 6h16v2.5H2V6Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><path d="M10 6v11" stroke="currentColor" stroke-width="1.4"/><path d="M10 6S8.4 3 6.8 3a1.5 1.5 0 0 0 0 3H10Zm0 0s1.6-3 3.2-3a1.5 1.5 0 0 1 0 3H10Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/></svg>';

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function dateOnly(ms) {
  return new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
}

/* ── The card ─────────────────────────────────────────────────  */

/* A slight, stable tilt per slot, so the stamps look hand-pressed
   without jumping about between visits. */
function tilt(cardNumber, index) {
  const seed = Math.sin(cardNumber * 97 + index * 31) * 10000;
  return Math.round((seed - Math.floor(seed)) * 24 - 12);
}

function stampCard(card, { label } = {}) {
  const full = card.stamps >= card.size;
  const root = el('div', 'stamp-card' + (full ? ' is-full' : ''));
  root.setAttribute('role', 'img');
  root.setAttribute('aria-label', 'Stamp card ' + card.number + ': ' + card.stamps + ' of ' + card.size + ' stamps');

  const head = el('div', 'stamp-card-head');
  head.appendChild(el('span', 'stamp-card-mark'));
  const titles = el('div', 'stamp-card-titles');
  titles.appendChild(el('div', 'stamp-card-brand', 'Bambee Rewards'));
  titles.appendChild(el('div', 'stamp-card-no', label || 'Card No. ' + String(card.number).padStart(2, '0')));
  head.appendChild(titles);
  head.appendChild(el('div', 'stamp-card-count', card.stamps + '/' + card.size));
  root.appendChild(head);

  const grid = el('div', 'stamp-grid');
  const fresh = [];
  for (let i = 0; i < card.size; i++) {
    const slot = el('div', 'stamp-slot' + (i === card.size - 1 ? ' is-goal' : ''));
    if (i === card.size - 1) slot.innerHTML = '<span class="stamp-num stamp-gift">' + GIFT + '</span>';
    else slot.appendChild(el('span', 'stamp-num', String(i + 1)));

    if (i < card.stamps) {
      slot.classList.add('is-stamped');
      const ink = el('span', 'stamp-ink');
      ink.style.setProperty('--tilt', tilt(card.number, i) + 'deg');
      slot.appendChild(ink);
      if (!reducedMotion && i >= (card.seen_stamps || 0)) {
        slot.classList.add('is-new');
        fresh.push(slot);
      }
    }
    grid.appendChild(slot);
  }
  root.appendChild(grid);

  root.appendChild(el('div', 'stamp-card-foot', 'One stamp for every delivered order'));

  return { root, fresh };
}

/* Presses the unseen stamps on one at a time, then records them as
   seen. Waits until the card is on screen, so a stamp landing below
   the fold isn't missed. */
function playStamps(card, root, fresh) {
  const done = () => {
    if (card.uuid && card.stamps > (card.seen_stamps || 0)) {
      Fetcher.post({ endpoint: '/v1/loyalty/card/' + card.uuid + '/seen', body: { stamps: card.stamps }, showError: false });
    }
  };
  if (!fresh.length) { done(); return; }

  const start = () => {
    fresh.forEach((slot, index) => {
      setTimeout(() => {
        slot.classList.remove('is-new');
        slot.classList.add('is-landing');
        root.classList.remove('is-thump');
        void root.offsetWidth;   // restart the thump for each stamp
        root.classList.add('is-thump');
      }, 350 + index * STAMP_GAP_MS);
    });
    const total = 350 + fresh.length * STAMP_GAP_MS + 500;
    setTimeout(() => {
      if (card.stamps >= card.size) root.classList.add('is-celebrating');
      done();
    }, total);
  };

  if (!('IntersectionObserver' in window)) { start(); return; }
  const watch = new IntersectionObserver((entries) => {
    if (entries.some((e) => e.isIntersecting)) {
      watch.disconnect();
      start();
    }
  }, { threshold: 0.5 });
  watch.observe(root);
}

/* ── Under the card ───────────────────────────────────────────  */

function progressLine(card) {
  const left = card.size - card.stamps;
  if (card.stamps === 0) {
    return card.number === 1
      ? 'Your first delivered order earns your first stamp.'
      : 'Your next delivered order starts this card.';
  }
  if (left === 1) return 'One more delivery fills your card.';
  return left + ' more deliveries to your reward.';
}

function codeBox(reward) {
  const box = el('div', 'reward-code');
  box.appendChild(el('code', '', reward.code));
  const copy = el('button', 'btn btn-ghost btn-sm', 'Copy');
  copy.type = 'button';
  copy.onclick = async () => {
    try {
      await navigator.clipboard.writeText(reward.code);
      toast.success('Code copied');
    } catch (e) {
      toast.info('Code: ' + reward.code);
    }
  };
  box.appendChild(copy);
  return box;
}

function fullCardPanel(card, rerender) {
  const panel = el('div', 'reward-panel');

  if (!card.reward_ready || !card.reward) {
    panel.classList.add('is-waiting');
    panel.appendChild(el('div', 'reward-panel-title', 'Stay tuned — your reward is coming soon'));
    panel.appendChild(el('p', 'reward-panel-text',
      'You filled the card! We are picking something special for you, and we will let you know the moment it is ready.'));
    return panel;
  }

  panel.appendChild(el('div', 'reward-panel-eyebrow', 'Your reward is ready'));
  panel.appendChild(el('div', 'reward-panel-title', card.reward.summary));
  if (card.reward.description) panel.appendChild(el('p', 'reward-panel-text', card.reward.description));
  if (card.reward.valid_days) {
    panel.appendChild(el('p', 'reward-panel-text',
      'Once redeemed, your code is valid for ' + card.reward.valid_days + (card.reward.valid_days === 1 ? ' day.' : ' days.')));
  }

  const redeem = el('button', 'btn btn-primary', 'Redeem my reward');
  redeem.type = 'button';
  redeem.onclick = async () => {
    redeem.disabled = true;
    const response = await Fetcher.post({ endpoint: '/v1/loyalty/card/' + card.uuid + '/redeem', body: {} });
    if (!response.ok) { redeem.disabled = false; return; }
    toast.success('Reward unlocked');
    rerender(response.data);
  };
  panel.appendChild(redeem);
  return panel;
}

function redeemedRow(card) {
  const reward = card.reward || {};
  const row = el('div', 'reward-row');

  const text = el('div', 'reward-row-text');
  text.appendChild(el('div', 'reward-row-title', reward.summary || 'Reward'));
  const bits = ['Card No. ' + String(card.number).padStart(2, '0')];
  if (card.redeemed_at) bits.push('redeemed ' + dateOnly(card.redeemed_at));
  if (reward.valid_until && !reward.used) bits.push((reward.expired ? 'expired ' : 'use by ') + dateOnly(reward.valid_until));
  text.appendChild(el('div', 'reward-row-sub', bits.join(' · ')));
  row.appendChild(text);

  if (reward.used) row.appendChild(el('span', 'tag tag-green', 'Used'));
  else if (reward.expired) row.appendChild(el('span', 'tag tag-red', 'Expired'));
  else if (reward.code) row.appendChild(codeBox(reward));
  return row;
}

/* ── Render ───────────────────────────────────────────────────  */

function section(title) {
  const box = el('section', 'rewards-section');
  if (title) box.appendChild(el('h2', 'rewards-section-title', title));
  return box;
}

function render(data) {
  host.innerHTML = '';
  const plays = [];

  // Full cards first: they are the ones with something to do.
  data.full.forEach((card) => {
    const box = section(null);
    const { root, fresh } = stampCard(card);
    box.appendChild(root);
    box.appendChild(fullCardPanel(card, (redeemed) => {
      // Swap this card for its revealed code, and keep the rest.
      data.full = data.full.filter((c) => c.uuid !== card.uuid);
      data.redeemed.unshift(redeemed);
      render(data);
      const code = host.querySelector('.reward-row .reward-code');
      if (code) code.closest('.reward-row').classList.add('is-new-reward');
    }));
    host.appendChild(box);
    plays.push([card, root, fresh]);
  });

  const current = data.current;
  const box = section(data.full.length ? 'Your next card' : null);
  const { root, fresh } = stampCard(current);
  box.appendChild(root);
  box.appendChild(el('p', 'stamp-progress', progressLine(current)));
  host.appendChild(box);
  plays.push([current, root, fresh]);

  if (data.redeemed.length) {
    const list = section('Your rewards');
    data.redeemed.forEach((card) => list.appendChild(redeemedRow(card)));
    host.appendChild(list);
  }

  plays.forEach(([card, cardRoot, slots]) => playStamps(card, cardRoot, slots));
}

(async function init() {
  const profile = await mountAccountChrome();
  if (!profile) return;

  const response = await Fetcher.get({ endpoint: '/v1/loyalty/cards' });
  if (!response.ok) {
    host.innerHTML = '';
    host.appendChild(el('p', 'stamp-progress', "Couldn't load your stamp card. Please try again in a moment."));
    return;
  }
  render(response.data);
})();
