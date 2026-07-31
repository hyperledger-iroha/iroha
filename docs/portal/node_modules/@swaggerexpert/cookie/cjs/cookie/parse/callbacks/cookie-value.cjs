"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../../apg-lite.cjs");
const cookieValue = (state, chars, phraseIndex, phraseLength, data) => {
  if (state === _apgLite.identifiers.SEM_PRE) {
    data.push(['cookie-value', _apgLite.utilities.charsToString(chars, phraseIndex, phraseLength)]);
  } else if (state === _apgLite.identifiers.SEM_POST) {
    /* not used in this example */
  }
  return _apgLite.identifiers.SEM_OK;
};
var _default = exports.default = cookieValue;