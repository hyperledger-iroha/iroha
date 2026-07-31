"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _DereferenceError = _interopRequireDefault(require("./DereferenceError.cjs"));
/**
 * @public
 */
class MaximumDereferenceDepthError extends _DereferenceError.default {}
var _default = exports.default = MaximumDereferenceDepthError;