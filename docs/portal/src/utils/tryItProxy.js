/**
 * Utility helpers for interacting with the Try It proxy from browser widgets.
 *
 * These functions are shared across Swagger UI, RapiDoc, and the embedded
 * console so all widgets stay aligned on how proxy URLs and bearer tokens are
 * handled.
 */

const PROXY_PREFIX = '/proxy';

/**
 * Trim whitespace and trailing slashes from a proxy base URL.
 *
 * @param {string|undefined|null} value
 * @returns {string}
 */
export function normaliseProxyBase(value) {
  const trimmed = typeof value === 'string' ? value.trim() : '';
  if (trimmed === '') {
    return '';
  }
  return trimmed.endsWith('/') ? trimmed.slice(0, -1) : trimmed;
}

/**
 * Build the proxied URL that the widgets should target.
 *
 * When the proxy is enabled, every upstream Torii request is rewritten to
 * `${proxyBase}/proxy/<path>?<query>`. If the incoming URL already targets the
 * proxy prefix we leave it untouched so manual overrides still work.
 *
 * @param {string} proxyBase
 * @param {string} originalUrl
 * @returns {string|null} proxied URL or null when the proxy is disabled
 */
export function buildProxiedUrl(proxyBase, originalUrl) {
  const base = normaliseProxyBase(proxyBase);
  if (base === '' || !originalUrl) {
    return null;
  }

  let pathname = '';
  let search = '';

  if (/^https?:\/\//i.test(originalUrl)) {
    const parsed = new URL(originalUrl);
    pathname = parsed.pathname || '/';
    search = parsed.search || '';
  } else {
    const [pathPart, queryPart] = originalUrl.split('?', 2);
    pathname = pathPart || '/';
    search = queryPart ? `?${queryPart}` : '';
  }

  if (!pathname.startsWith('/')) {
    pathname = `/${pathname}`;
  }

  if (pathname.startsWith(`${PROXY_PREFIX}/`) || pathname === PROXY_PREFIX) {
    return `${base}${pathname}${search}`;
  }

  return `${base}${PROXY_PREFIX}${pathname}${search}`;
}

/**
 * Convert arbitrary user input into the header value forwarded to the proxy.
 * The proxy normalises bearer formats, so we only remove surrounding whitespace.
 *
 * @param {string} value
 * @returns {string}
 */
export function formatProxyAuth(value) {
  if (typeof value !== 'string') {
    return '';
  }
  return value.trim();
}
