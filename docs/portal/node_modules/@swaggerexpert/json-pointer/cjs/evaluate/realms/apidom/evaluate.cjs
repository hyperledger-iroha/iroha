"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("../../../evaluate/index.cjs"));
var _realm = _interopRequireDefault(require("./realm.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const evaluate = (value, jsonPointer, options = {}) => {
  return (0, _index.default)(value, jsonPointer, {
    ...options,
    realm: new _realm.default()
  });
};
var _default = exports.default = evaluate;