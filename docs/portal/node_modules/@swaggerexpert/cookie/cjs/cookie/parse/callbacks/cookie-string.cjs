"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../../apg-lite.cjs");
const cookieString = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === _apgLite.identifiers.SEM_PRE) {
    if (Array.isArray(data) === false) {
      throw new Error("parser's user data must be an array");
    }
    data.push(['cookie-string', _apgLite.utilities.charsToString(chars, phraseIndex, phraseLength)]);
  }
  return _apgLite.identifiers.SEM_OK;
};
var _default = exports.default = cookieString;