/* ═══════════════════════════════════════════════════════════════
   Bambee Web — review-form.js

   `POST /v1/review` has been live the whole time with nothing
   calling it: the product page's "Write a Review" button used to
   send people to their orders, and the orders page's own button
   was wireframe markup. So a customer could not review anything.

   Two surfaces open the same composer, which is why it lives here:

   - the product page, which only offers it when the caller has a
     delivered order with this product still unreviewed on it
     (GET /v1/review/eligible);
   - the orders page, which asks whether the review is for one item
     or for the whole order.

   Only buyers can review, so every review names its order, and the
   backend checks the order is the caller's own and delivered. A
   whole-order review is posted without a `product_id`; the server
   writes it onto every product on the order not yet reviewed, and
   each of those products' ratings moves with it.
═══════════════════════════════════════════════════════════════ */
import { Fetcher } from './fetcher.js';
import { toast } from './toast.js';
import { session } from './session.js';
import { openModal, field, textInput, textArea } from './modal.js';
import { paintImage } from './catalog.js';

const STAR_PATH = 'M6 1l1.1 3.4H11L8.2 6.5l1 3.4L6 8.1 2.8 9.9l1-3.4L1 4.4h3.9L6 1Z';
const RATING_WORD = ['', 'Poor', 'Fair', 'Good', 'Very good', 'Excellent'];

/* ── Star picker ──────────────────────────────────────────────
   Keyboard-operable, because it is the one required field and a
   mouse-only control makes the form impossible to complete from
   the keyboard.                                                */
function starPicker(onChange) {
  const wrap = document.createElement('div');
  wrap.style.cssText = 'display:flex;align-items:center;gap:10px;';

  const row = document.createElement('div');
  row.className = 'stars';
  row.setAttribute('role', 'radiogroup');
  row.setAttribute('aria-label', 'Rating');
  row.style.cssText = 'display:flex;gap:4px;cursor:pointer;';

  const word = document.createElement('span');
  word.style.cssText = 'font-size:13px;color:var(--subtle);';

  let value = 0;

  const paint = (upTo) => {
    [...row.children].forEach((star, index) => {
      star.querySelector('path').setAttribute('fill', index < upTo ? '#C8A772' : '#E8E5E0');
    });
    word.textContent = RATING_WORD[upTo] || '';
  };

  const select = (i) => {
    value = i;
    paint(i);
    [...row.children].forEach((star, index) => {
      star.setAttribute('aria-checked', String(index + 1 === i));
      star.setAttribute('tabindex', index + 1 === i ? '0' : '-1');
    });
    onChange(i);
  };

  for (let i = 1; i <= 5; i++) {
    const star = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    star.setAttribute('viewBox', '0 0 12 12');
    star.setAttribute('width', '24');
    star.setAttribute('height', '24');
    star.setAttribute('role', 'radio');
    star.setAttribute('tabindex', i === 1 ? '0' : '-1');
    star.setAttribute('aria-label', i === 1 ? '1 star' : i + ' stars');
    star.setAttribute('aria-checked', 'false');
    star.innerHTML = '<path d="' + STAR_PATH + '" fill="#E8E5E0"/>';

    star.addEventListener('click', () => select(i));
    star.addEventListener('mouseenter', () => paint(i));
    star.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); select(i); }
      if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
        e.preventDefault();
        const next = Math.min(5, Math.max(1, (value || 1) + (e.key === 'ArrowRight' ? 1 : -1)));
        select(next);
        row.children[next - 1].focus();
      }
    });

    row.appendChild(star);
  }

  row.addEventListener('mouseleave', () => paint(value));

  wrap.appendChild(row);
  wrap.appendChild(word);
  return { el: wrap, get value() { return value; } };
}

/* ── The composer ─────────────────────────────────────────────  */

/**
 * @param {string} options.orderId        the delivered order the review comes from
 * @param {string} [options.productId]    omitted for a review of the whole order
 * @param {string} [options.productTitle] shown at the top, so a picker's choice stays visible
 * @param {*}      [options.image]        ImageStruct or bare id
 * @param {Array}  [options.items]        the order lines a whole-order review covers
 * @param {Function} [options.onDone]
 */
export function openReviewComposer({ orderId, productId, productTitle, image, items, onDone }) {
  if (!session.isSignedIn()) {
    toast.info('Sign in to write a review');
    setTimeout(() => session.requireSignedIn(), 700);
    return;
  }

  const form = document.createElement('div');
  const whole = !productId;

  if (whole) {
    form.appendChild(wholeOrderHead(items || []));
  } else if (productTitle) {
    const head = document.createElement('div');
    head.style.cssText = 'display:flex;gap:12px;align-items:center;padding-bottom:16px;'
      + 'margin-bottom:18px;border-bottom:1px solid var(--border);';

    const thumb = document.createElement('div');
    thumb.style.cssText = 'width:44px;height:44px;border-radius:var(--radius-sm);'
      + 'background:var(--input-bg);flex-shrink:0;';
    paintImage(thumb, image);

    const name = document.createElement('div');
    name.style.cssText = 'font-size:13.5px;font-weight:600;color:var(--dark);line-height:1.4;';
    name.textContent = productTitle;

    head.appendChild(thumb);
    head.appendChild(name);
    form.appendChild(head);
  }

  const error = document.createElement('div');
  error.style.cssText = 'font-size:12.5px;color:var(--error);display:none;margin-top:-6px;';

  const picker = starPicker(() => { error.style.display = 'none'; });
  form.appendChild(field('Your rating', picker.el));

  const title = textInput({ placeholder: 'Sum it up in a few words (optional)', maxLength: 120 });
  form.appendChild(field('Headline', title));

  const body = textArea({ placeholder: 'What did you like, or not like, about it?', rows: 5 });
  form.appendChild(field('Your review', body, whole
    ? 'This review will show on each of these products and count towards their ratings.'
    : 'Your review will show on the product page.'));

  form.appendChild(error);

  const { close } = openModal({
    title: whole ? 'Review your order' : 'Write a review',
    body: form,
    width: '520px',
    actions: [
      { label: 'Cancel', className: 'btn-ghost', onClick: (done) => done() },
      {
        label: 'Submit review',
        className: 'btn-primary',
        primary: true,
        onClick: async (done, button) => {
          error.style.display = 'none';

          if (!picker.value) {
            error.textContent = 'Pick a rating first.';
            error.style.display = '';
            return;
          }
          if (!body.value.trim()) {
            error.textContent = 'Tell other shoppers what you thought.';
            error.style.display = '';
            return;
          }

          button.disabled = true;
          const response = await Fetcher.post({
            endpoint: '/v1/review',
            body: {
              order_id: orderId,
              product_id: productId || null,
              rating: picker.value,
              title: title.value.trim() || null,
              body: body.value.trim(),
            },
          });
          button.disabled = false;

          if (!response.ok) return;

          done();
          toast.success('Thanks — your review is live');
          if (onDone) onDone();
        },
      },
    ],
  });

  return close;
}

/* "3 items from this order" with a strip of their pictures. */
function wholeOrderHead(items) {
  const head = document.createElement('div');
  head.style.cssText = 'padding-bottom:16px;margin-bottom:18px;border-bottom:1px solid var(--border);';

  const strip = document.createElement('div');
  strip.style.cssText = 'display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap;';
  items.forEach((item) => {
    const thumb = document.createElement('div');
    thumb.title = item.title;
    thumb.style.cssText = 'width:44px;height:44px;border-radius:var(--radius-sm);'
      + 'background:var(--input-bg);flex-shrink:0;';
    paintImage(thumb, item.image_id || item.image);
    strip.appendChild(thumb);
  });

  const label = document.createElement('div');
  label.style.cssText = 'font-size:13.5px;font-weight:600;color:var(--dark);line-height:1.4;';
  label.textContent = items.length === 1 ? '1 item from this order' : items.length + ' items from this order';

  head.append(strip, label);
  return head;
}

function pickerRow({ thumbFrom, text, note, disabled, onClick }) {
  const row = document.createElement('div');
  row.style.cssText = 'display:flex;gap:12px;align-items:center;padding:10px;'
    + 'border:1px solid var(--border);border-radius:var(--radius-sm);'
    + (disabled ? 'opacity:.55;cursor:default;' : 'cursor:pointer;');

  const thumb = document.createElement('div');
  thumb.style.cssText = 'width:40px;height:40px;border-radius:var(--radius-sm);'
    + 'background:var(--input-bg);flex-shrink:0;display:flex;align-items:center;justify-content:center;';
  if (thumbFrom) paintImage(thumb, thumbFrom);
  else thumb.innerHTML = '<svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="M2.5 6.2 10 3l7.5 3.2v7.6L10 17l-7.5-3.2V6.2Z" stroke="var(--primary-text)" stroke-width="1.4" stroke-linejoin="round"/><path d="m2.5 6.2 7.5 3.3 7.5-3.3M10 9.5V17" stroke="var(--primary-text)" stroke-width="1.4" stroke-linejoin="round"/></svg>';

  const name = document.createElement('div');
  name.style.cssText = 'flex:1;font-size:13px;color:var(--dark);line-height:1.4;';
  name.textContent = text;

  row.append(thumb, name);
  if (note) {
    const tag = document.createElement('span');
    tag.style.cssText = 'font-size:11.5px;font-weight:700;color:var(--success);white-space:nowrap;';
    tag.textContent = note;
    row.appendChild(tag);
  }

  if (!disabled) {
    row.setAttribute('role', 'button');
    row.tabIndex = 0;
    row.addEventListener('click', onClick);
    row.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); row.click(); }
    });
  }
  return row;
}

/*
  Asks what on an order is being reviewed: one item, or the whole
  order at once. Items already reviewed from this order are shown but
  can't be picked again. An order with a single product skips the
  question.

  `onPick(item)` for one item; `onPick(null, items)` for the whole
  order, with the items that review will land on.
*/
export async function openOrderReviewPicker(order, onPick) {
  // One entry per product — two shades of one lipstick are one review.
  const seen = new Set();
  const products = (order.items || []).filter((item) => {
    if (seen.has(item.product_id)) return false;
    seen.add(item.product_id);
    return true;
  });
  if (!products.length) {
    toast.info('Nothing on this order to review');
    return;
  }

  const response = await Fetcher.get({
    endpoint: '/v1/review/reviewed',
    query: { order_id: order.uuid },
  });
  if (!response.ok) return;
  const reviewed = new Set(response.data || []);
  const pending = products.filter((item) => !reviewed.has(item.product_id));

  if (!pending.length) {
    toast.info("You've reviewed everything on this order");
    return;
  }
  if (products.length === 1) {
    onPick(pending[0]);
    return;
  }

  const list = document.createElement('div');
  list.style.cssText = 'display:flex;flex-direction:column;gap:8px;';

  if (pending.length > 1) {
    list.appendChild(pickerRow({
      text: 'The whole order — ' + pending.length + ' items, one review',
      onClick: () => { close(); onPick(null, pending); },
    }));
    const or = document.createElement('div');
    or.style.cssText = 'font-size:12px;color:var(--subtle);margin:6px 0 2px;';
    or.textContent = 'Or review a single item';
    list.appendChild(or);
  }

  products.forEach((item) => {
    const done = reviewed.has(item.product_id);
    list.appendChild(pickerRow({
      thumbFrom: item.image_id || item.image,
      text: item.title,
      note: done ? 'Reviewed' : null,
      disabled: done,
      onClick: () => { close(); onPick(item); },
    }));
  });

  const { close } = openModal({
    title: 'What are you reviewing?',
    body: list,
    width: '460px',
  });
}
