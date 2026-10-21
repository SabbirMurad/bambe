/*
  HTTP Header - Content Security Policy
  Read - https://infosec.mozilla.org/guidelines/web_security#content-security-policy for more details

  This header used to be emitted with an EMPTY value, which does nothing at all
  while looking configured. The directives below are derived from what the
  templates in `pages/` and the scripts in `assets/js/` actually load - adjust
  them when you add a CDN, an analytics script, or an external API.
*/

/// Directives, one per line so a diff shows exactly what changed.
///
/// Notes on the less obvious choices:
///   - `script-src` allows `'unsafe-inline'` because the storefront pages carry
///     small inline `<script>` blocks (nav wiring, page boot). Moving those into
///     files is what would let this be dropped - it is most of a CSP's XSS value.
///     JSON-LD blocks are data, not script, and need nothing here.
///   - `style-src` allows `'unsafe-inline'` for the many `style=""` attributes and
///     for Google Fonts' stylesheet from fonts.googleapis.com.
///   - `img-src` allows `blob:` and `data:`: blur-hash placeholders are data URIs,
///     and the admin's CSV exports are blob downloads.
///   - `frame-ancestors 'self'` matches X-Frame-Options: SAMEORIGIN - the site
///     may frame its own pages (responsive previews), nobody else may.
const DIRECTIVES: &[&str] = &[
    "default-src 'self'",
    "base-uri 'self'",
    "form-action 'self'",
    "frame-ancestors 'self'",
    "object-src 'none'",
    "script-src 'self' 'unsafe-inline'",
    "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com",
    "font-src 'self' data: https://fonts.gstatic.com",
    "img-src 'self' data: blob:",
    "media-src 'self' blob:",
    "connect-src 'self'",
];

pub fn get_policy() -> (String, String) {
    ("Content-Security-Policy".to_owned(), DIRECTIVES.join("; "))
}
