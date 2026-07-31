"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieName = _interopRequireDefault(require("../../../cookie/test/cookie-name.cjs"));
var _utils = require("../../../utils.cjs");
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const cookieNameStrictPercentEncoder = cookieName => {
  const value = String(cookieName);
  let result = '';
  for (const char of value) {
    result += (0, _cookieName.default)(char) ? char : (0, _utils.percentEncodeChar)(char);
  }
  return result;
};
var _default = exports.default = cookieNameStrictPercentEncoder;