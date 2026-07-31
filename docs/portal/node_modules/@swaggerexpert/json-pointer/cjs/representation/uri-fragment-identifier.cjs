"use strict";

exports.__esModule = true;
exports.to = exports.fromURIReference = exports.from = void 0;
const to = jsonPointer => {
  const encodedFragment = [...jsonPointer].map(char => /^[A-Za-z0-9\-._~!$&'()*+,;=:@/?]$/.test(char) ? char : encodeURIComponent(char)).join('');
  return `#${encodedFragment}`;
};
exports.to = to;
const from = fragment => {
  try {
    const rfc3986Fragment = fragment.startsWith('#') ? fragment.slice(1) : fragment;
    return decodeURIComponent(rfc3986Fragment);
  } catch {
    return fragment;
  }
};
exports.from = from;
const fromURIReference = uriReference => {
  const fragmentIndex = uriReference.indexOf('#');
  const fragment = fragmentIndex === -1 ? '#' : uriReference.substring(fragmentIndex);
  return from(fragment);
};
exports.fromURIReference = fromURIReference;