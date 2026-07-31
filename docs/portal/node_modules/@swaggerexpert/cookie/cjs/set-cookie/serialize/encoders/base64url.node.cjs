"use strict";

exports.__esModule = true;
exports.default = void 0;
var _utils = require("../../../utils.cjs");
var _base64Node = _interopRequireDefault(require("./base64.node.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const base64urlEncoder = input => {
  return (0, _utils.toBase64url)((0, _base64Node.default)(input));
};
var _default = exports.default = base64urlEncoder;