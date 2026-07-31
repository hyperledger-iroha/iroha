"use strict";

exports.__esModule = true;
exports.unquote = exports.toBase64url = exports.toBase64 = exports.quote = exports.percentEncodeChar = exports.noop = exports.isQuoted = exports.identity = void 0;
const percentEncodeChar = char => {
  if (typeof char !== 'string' || [...char].length !== 1) {
    throw new TypeError('Input must be a single character string.');
  }
  const codePoint = char.codePointAt(0);
  if (codePoint <= 0x7f) {
    // ASCII range: Encode as %XX
    return `%${codePoint.toString(16).toUpperCase().padStart(2, '0')}`;
  } else {
    // Non-ASCII range: Use encodeURIComponent to handle UTF-8 encoding
    return encodeURIComponent(char);
  }
};
exports.percentEncodeChar = percentEncodeChar;
const toBase64url = base64 => {
  return base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
};
exports.toBase64url = toBase64url;
const toBase64 = base64url => {
  const base64 = base64url.replace(/-/g, '+').replace(/_/g, '/');
  const padding = '='.repeat((4 - base64.length % 4) % 4);
  return `${base64}${padding}`;
};
exports.toBase64 = toBase64;
const isQuoted = value => {
  return value.length >= 2 && value.startsWith('"') && value.endsWith('"');
};
exports.isQuoted = isQuoted;
const unquote = value => {
  return isQuoted(value) ? value.slice(1, -1) : value;
};
exports.unquote = unquote;
const quote = value => {
  return `"${value}"`;
};
exports.quote = quote;
const identity = a => a;
exports.identity = identity;
const noop = () => {};
exports.noop = noop;