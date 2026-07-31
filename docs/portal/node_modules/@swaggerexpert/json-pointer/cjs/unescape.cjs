"use strict";

exports.__esModule = true;
exports.default = void 0;
const unescape = referenceToken => {
  if (typeof referenceToken !== 'string') {
    throw new TypeError('Reference token must be a string');
  }
  return referenceToken.replace(/~1/g, '/').replace(/~0/g, '~');
};
var _default = exports.default = unescape;