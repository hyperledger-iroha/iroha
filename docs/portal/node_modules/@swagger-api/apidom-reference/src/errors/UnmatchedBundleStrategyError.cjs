"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _BundleError = _interopRequireDefault(require("./BundleError.cjs"));
/**
 * @public
 */
class UnmatchedBundleStrategyError extends _BundleError.default {}
var _default = exports.default = UnmatchedBundleStrategyError;