"use strict";

exports.__esModule = true;
exports.default = void 0;
var _utils = require("../../../utils.cjs");
var _base64Node = _interopRequireDefault(require("./base64.node.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const base64urlDecoder = input => {
  const base64 = (0, _utils.toBase64)(input);
  return (0, _base64Node.default)(base64);
};
var _default = exports.default = base64urlDecoder;