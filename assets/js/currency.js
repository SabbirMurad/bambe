/* ── Currency ─────────────────────────────────────────────────
   The store's currency, as the server stamped it on <html>
   (data-currency, from the store settings), and how to write an
   amount in it. Every price on the site goes through here, so the
   symbol is never written into a script.

   Amounts are minor units (`Cents`, paisa for BDT) until the last
   moment. Taka is written without paisa when there are none —
   "৳1,200", the way prices are shown in Bangladesh. */

const SYMBOLS = { BDT: '৳', USD: '$', GBP: '£', EUR: '€', CAD: 'CA$', AUD: 'A$', JPY: '¥', INR: '₹' };

// Shown without decimals when the amount is whole.
const WHOLE_UNITS = new Set(['BDT', 'JPY', 'INR']);

export function currencyCode() {
  return (document.documentElement.dataset.currency || 'BDT').toUpperCase();
}

export function currencySymbol() {
  const code = currencyCode();
  return SYMBOLS[code] || code + ' ';
}

/* Cents in, "৳1,200" or "৳1,200.50" out. Rounded first: some figures
   (averages) arrive with fractions of a cent. */
export function formatMoney(cents) {
  const sign = cents < 0 ? '-' : '';
  const abs = Math.round(Math.abs(cents));
  const whole = Math.floor(abs / 100).toLocaleString('en-US');
  const part = abs % 100;
  const fraction = part === 0 && WHOLE_UNITS.has(currencyCode()) ? '' : '.' + String(part).padStart(2, '0');
  return sign + currencySymbol() + whole + fraction;
}

/* Whole units in (the price filter works in them), "৳1,200" out. */
export function formatWhole(units) {
  return currencySymbol() + Math.round(units).toLocaleString('en-US');
}
