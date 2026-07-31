"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _UnsupportedOperationError = _interopRequireDefault(require("./UnsupportedOperationError.cjs"));
/**
 * @public
 */
class NotImplementedError extends _UnsupportedOperationError.default {}
var _default = exports.default = NotImplementedError;