"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieName = _interopRequireDefault(require("../../../cookie/test/cookie-name.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieNameStrictValidator = cookieName => {
  if (!(0, _cookieName.default)(cookieName)) {
    throw new TypeError(`Invalid cookie name: ${cookieName}`);
  }
};
var _default = exports.default = cookieNameStrictValidator;