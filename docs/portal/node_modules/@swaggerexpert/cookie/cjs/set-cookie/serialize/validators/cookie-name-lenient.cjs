"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieName = _interopRequireDefault(require("../../../cookie/test/cookie-name.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieNameLenientValidator = cookieName => {
  if (!(0, _cookieName.default)(cookieName, {
    strict: false
  })) {
    throw new TypeError(`Invalid cookie name: ${cookieName}`);
  }
};
var _default = exports.default = cookieNameLenientValidator;