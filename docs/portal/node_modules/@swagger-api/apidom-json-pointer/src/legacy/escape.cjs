"use strict";

exports.__esModule = true;
exports.default = void 0;
var _jsonPointer = require("@swaggerexpert/json-pointer");
/**
 * @public
 * @deprecated
 */
const escape = referenceToken => {
  return _jsonPointer.URIFragmentIdentifier.to((0, _jsonPointer.escape)(referenceToken)).slice(1);
};
var _default = exports.default = escape;