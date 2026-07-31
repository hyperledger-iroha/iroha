"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../../../apg-lite.cjs");
var _grammar = _interopRequireDefault(require("../../../grammar.cjs"));
var _utils = require("../../../utils.cjs");
var _cookieValue = _interopRequireDefault(require("../../../cookie/test/cookie-value.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const parser = new _apgLite.Parser();
const grammar = new _grammar.default();
const cookieValueStrictPercentEncoder = cookieValue => {
  const value = String(cookieValue);

  // return early if the value is already valid
  if ((0, _cookieValue.default)(value)) return value;

  // detect if the value is quoted
  const isQuoted = (0, _utils.isQuoted)(value);

  // remove quotes if present for processing
  const valueToEncode = isQuoted ? (0, _utils.unquote)(value) : value;
  let result = '';
  for (const char of valueToEncode) {
    result += parser.parse(grammar, 'cookie-octet', char).success ? char : (0, _utils.percentEncodeChar)(char);
  }

  // return quoted if input was quoted, unquoted otherwise
  return isQuoted ? (0, _utils.quote)(result) : result;
};
var _default = exports.default = cookieValueStrictPercentEncoder;