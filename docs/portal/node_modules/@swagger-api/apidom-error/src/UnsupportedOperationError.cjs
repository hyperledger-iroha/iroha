"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ApiDOMError = _interopRequireDefault(require("./ApiDOMError.cjs"));
/**
 * @public
 */
class UnsupportedOperationError extends _ApiDOMError.default {}
var _default = exports.default = UnsupportedOperationError;