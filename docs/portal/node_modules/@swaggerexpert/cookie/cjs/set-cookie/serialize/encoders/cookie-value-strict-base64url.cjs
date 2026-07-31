"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieValueStrictBase = _interopRequireDefault(require("./cookie-value-strict-base64.cjs"));
var _base64urlNode = _interopRequireDefault(require("./base64url.node.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieValueStrictBase64urlEncoder = cookieValue => {
  return (0, _cookieValueStrictBase.default)(cookieValue, _base64urlNode.default);
};
var _default = exports.default = cookieValueStrictBase64urlEncoder;