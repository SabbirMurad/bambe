/* ═══════════════════════════════════════════════════════════════
   Bambee Web — session.js

   Everything about who the caller is: the tokens from sign-in, the
   guest id a signed-out cart is keyed on, and the refresh handshake.

   fetcher.js imports this to stamp `Authorization` on every request,
   so nothing else has to remember to. Pages import it to guard
   themselves and to render the signed-in/signed-out nav.

   Deliberately does not import fetcher.js: the refresh call has to be
   a raw fetch, or a 401 on refresh would recurse back into itself.
═══════════════════════════════════════════════════════════════ */

const AUTH_KEY  = 'bambe.auth';
const GUEST_KEY = 'bambe.guest';

// Refresh this long before the token actually dies, so a request that
// takes a moment to reach the server doesn't arrive already expired.
const EXPIRY_SKEW_MS = 30 * 1000;

function uuid() {
  if (window.crypto && crypto.randomUUID) return crypto.randomUUID();
  // Older Safari / non-secure contexts. Only ever used as a cart key.
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
    const r = Math.random() * 16 | 0;
    return (c === 'x' ? r : (r & 0x3 | 0x8)).toString(16);
  });
}

class Session {
  /* ── The signed-in user ─────────────────────────────────── */

  /** The stored auth payload, or null when signed out. */
  read() {
    try {
      const raw = localStorage.getItem(AUTH_KEY);
      return raw ? JSON.parse(raw) : null;
    } catch (e) {
      return null;
    }
  }

  /*
    Takes the `auth_payload` from sign-in / admin sign-in, or the flat
    body from validate-email — both carry the same five fields, so
    callers can hand over whichever they got.
  */
  save(payload) {
    if (!payload || !payload.access_token) return null;
    const data = {
      access_token: payload.access_token,
      access_token_valid_till: payload.access_token_valid_till,
      refresh_token: payload.refresh_token,
      user_id: payload.user_id,
      role: payload.role,
    };
    localStorage.setItem(AUTH_KEY, JSON.stringify(data));
    // A signed-in cart is keyed on the user, so the guest id has served
    // its purpose — the caller merges first, then this drops it.
    return data;
  }

  clear() {
    localStorage.removeItem(AUTH_KEY);
  }

  token()      { const a = this.read(); return a ? a.access_token : null; }
  userId()     { const a = this.read(); return a ? a.user_id : null; }
  role()       { const a = this.read(); return a ? a.role : null; }
  isSignedIn() { return !!this.token(); }
  isAdmin()    { return this.role() === 'Administrator'; }

  /** True when the access token is gone or about to expire. */
  isExpired() {
    const auth = this.read();
    if (!auth) return true;
    if (!auth.access_token_valid_till) return false;
    return Date.now() >= auth.access_token_valid_till - EXPIRY_SKEW_MS;
  }

  /*
    Swaps the refresh token for a fresh access token. Returns the new
    token, or null if the refresh itself failed — in which case the
    session is cleared, because a refresh token the server rejects is
    not going to start working.

    Concurrent callers share one in-flight request: a page that fires
    four fetches at load would otherwise burn four refreshes and race
    over which result lands last.
  */
  refresh() {
    if (this._refreshing) return this._refreshing;

    const auth = this.read();
    if (!auth || !auth.refresh_token) return Promise.resolve(null);

    const domain = window.projectDomain || '';
    this._refreshing = fetch(domain + '/api/auth/refresh', {
      method: 'POST',
      headers: { 'Accept': 'application/json', 'Content-Type': 'application/json' },
      body: JSON.stringify({
        refresh_token: auth.refresh_token,
        user_id: auth.user_id,
        role: auth.role,
      }),
    })
      .then((res) => (res.ok ? res.json() : null))
      .then((body) => {
        if (!body || !body.access_token) {
          this.clear();
          return null;
        }
        const next = this.read() || auth;
        next.access_token = body.access_token;
        next.access_token_valid_till = body.access_token_valid_till;
        localStorage.setItem(AUTH_KEY, JSON.stringify(next));
        return body.access_token;
      })
      .catch(() => {
        this.clear();
        return null;
      })
      .finally(() => { this._refreshing = null; });

    return this._refreshing;
  }

  /*
    Page guard. Call at the top of anything that cannot render signed
    out; sends the caller to login with a return path so they land back
    where they were trying to go.
  */
  requireSignedIn(loginPath) {
    if (this.isSignedIn()) return true;
    const next = encodeURIComponent(location.pathname + location.search);
    location.href = (loginPath || '/login') + '?next=' + next;
    return false;
  }

  requireAdmin() {
    if (this.isAdmin()) return true;
    const next = encodeURIComponent(location.pathname + location.search);
    location.href = '/admin/login?next=' + next;
    return false;
  }

  /* ── The signed-out cart ────────────────────────────────── */

  /*
    `resolve_owner` (src/handler/cart.rs) keys a guest cart on the
    X-Guest-Session header. Minted on first read rather than at load,
    so a visitor who never opens the cart never gets an id.
  */
  guestId() {
    let id = localStorage.getItem(GUEST_KEY);
    if (!id) {
      id = uuid();
      localStorage.setItem(GUEST_KEY, id);
    }
    return id;
  }

  /** The id only if one was already minted — never creates one. */
  peekGuestId() {
    return localStorage.getItem(GUEST_KEY);
  }

  dropGuestId() {
    localStorage.removeItem(GUEST_KEY);
  }
}

export const session = new Session();
export default session;

window.addEventListener('load', () => {
  window.session = session;
});
