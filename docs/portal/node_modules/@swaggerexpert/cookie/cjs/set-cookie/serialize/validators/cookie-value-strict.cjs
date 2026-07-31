"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieValue = _interopRequireDefault(require("../../../cookie/test/cookie-value.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieValueStrictValidator = cookieValue => {
  if (!(0, _cookieValue.default)(cookieValue)) {
    throw new TypeError(`Invalid cookie value: ${cookieValue}`);
  }
};
var _default = exports.default = cookieValueStrictValidator;