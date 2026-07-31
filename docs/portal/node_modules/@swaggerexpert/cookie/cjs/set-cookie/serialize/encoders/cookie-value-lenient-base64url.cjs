"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieValueLenientBase = _interopRequireDefault(require("./cookie-value-lenient-base64.cjs"));
var _base64urlNode = _interopRequireDefault(require("./base64url.node.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieValueLenientBase64urlEncoder = cookieValue => {
  return (0, _cookieValueLenientBase.default)(cookieValue, _base64urlNode.default);
};
var _default = exports.default = cookieValueLenientBase64urlEncoder;