"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("../../set-cookie/serialize/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const serialize = (cookiePairs, options = {}) => {
  const cookiePairEntries = Array.isArray(cookiePairs) ? cookiePairs : typeof cookiePairs === 'object' && cookiePairs !== null ? Object.entries(cookiePairs) : [];
  return cookiePairEntries.map(([name, value]) => (0, _index.default)(name, value, options)).join('; ');
};
var _default = exports.default = serialize;