"use strict";

exports.__esModule = true;
exports.default = void 0;
var _JSONPointerError = _interopRequireDefault(require("./JSONPointerError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class JSONPointerCompileError extends _JSONPointerError.default {}
var _default = exports.default = JSONPointerCompileError;