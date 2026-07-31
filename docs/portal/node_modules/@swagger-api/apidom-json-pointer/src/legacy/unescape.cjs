"use strict";

exports.__esModule = true;
exports.default = void 0;
var _jsonPointer = require("@swaggerexpert/json-pointer");
/**
 * @public
 * @deprecated
 */
const unescape = referenceToken => {
  return _jsonPointer.URIFragmentIdentifier.from((0, _jsonPointer.unescape)(referenceToken));
};
var _default = exports.default = unescape;