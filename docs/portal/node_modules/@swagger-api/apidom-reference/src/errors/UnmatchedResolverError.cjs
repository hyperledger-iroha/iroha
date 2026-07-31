"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ResolverError = _interopRequireDefault(require("./ResolverError.cjs"));
/**
 * @public
 */
class UnmatchedResolverError extends _ResolverError.default {}
var _default = exports.default = UnmatchedResolverError;