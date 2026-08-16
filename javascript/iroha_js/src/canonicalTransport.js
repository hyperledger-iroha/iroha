/** Return the WHATWG URL wire spelling of one caller-supplied query. */
export function preparedTransportQuery(query) {
  if (query === undefined || query === null) {
    return undefined;
  }
  const raw = query instanceof URLSearchParams ? query.toString() : String(query);
  const url = new URL(`https://canonical.invalid/?${raw}`);
  if (url.hash) {
    throw new TypeError("canonical request query must not contain a URL fragment");
  }
  return url.search.slice(1);
}
