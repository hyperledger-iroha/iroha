"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieValue = _interopRequireDefault(require("../../../cookie/test/cookie-value.cjs"));
var _base64Node = _interopRequireDefault(require("./base64.node.cjs"));
var _utils = require("../../../utils.cjs");
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieValueStrictBase64Encoder = (cookieValue, encoder = _base64Node.default) => {
  const value = String(cookieValue);

  // return early if the value is already valid
  if ((0, _cookieValue.default)(value)) return value;

  // detect if the value is quoted
  const isQuoted = (0, _utils.isQuoted)(value);

  // remove quotes if present for processing
  const valueToEncode = isQuoted ? (0, _utils.unquote)(value) : value;

  // base64 encode the value
  const base64EncodedValue = encoder(valueToEncode);

  // return quoted if input was quoted, unquoted otherwise
  return isQuoted ? (0, _utils.quote)(base64EncodedValue) : base64EncodedValue;
};
var _default = exports.default = cookieValueStrictBase64Encoder;