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
class EvaluationJsonPointerError extends _JsonPointerError.default {
  pointer;
  tokens;
  failedToken;
  failedTokenPosition;
  element;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.pointer = structuredOptions.pointer;
      if (Array.isArray(structuredOptions.tokens)) {
        this.tokens = [...structuredOptions.tokens];
      }
      this.failedToken = structuredOptions.failedToken;
      this.failedTokenPosition = structuredOptions.failedTokenPosition;
      this.element = structuredOptions.element;
    }
  }
}
var _default = exports.default = EvaluationJsonPointerError;