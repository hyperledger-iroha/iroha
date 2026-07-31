"use strict";

exports.__esModule = true;
exports.default = void 0;
var _JSONPointerEvaluateError = _interopRequireDefault(require("./JSONPointerEvaluateError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
class JSONPointerTypeError extends _JSONPointerEvaluateError.default {}
var _default = exports.default = JSONPointerTypeError;