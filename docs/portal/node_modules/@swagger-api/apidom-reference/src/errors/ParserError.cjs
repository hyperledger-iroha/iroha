"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ParseError = _interopRequireDefault(require("./ParseError.cjs"));
/**
 * @public
 */
class ParserError extends _ParseError.default {}
var _default = exports.default = ParserError;