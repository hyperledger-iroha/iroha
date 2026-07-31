"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JsonPointerError = _interopRequireDefault(require("./JsonPointerError.cjs"));
/**
 * @public
 * @deprecated
 */

/**
 * @public
 * @deprecated
 */
class InvalidJsonPointerError extends _JsonPointerError.default {
  pointer;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.pointer = structuredOptions.pointer;
    }
  }
}
var _default = exports.default = InvalidJsonPointerError;