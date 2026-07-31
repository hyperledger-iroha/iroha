"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonSchema$anchorError = _interopRequireDefault(require("./JsonSchema$anchorError.cjs"));
/**
 * @public
 */
class InvalidJsonSchema$anchorError extends _JsonSchema$anchorError.default {
  constructor(anchor) {
    super(`Invalid JSON Schema $anchor "${anchor}".`);
  }
}
var _default = exports.default = InvalidJsonSchema$anchorError;