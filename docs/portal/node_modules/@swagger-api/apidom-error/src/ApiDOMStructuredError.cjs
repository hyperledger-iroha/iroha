"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ApiDOMError = _interopRequireDefault(require("./ApiDOMError.cjs"));
/**
 * @public
 */
class ApiDOMStructuredError extends _ApiDOMError.default {
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (structuredOptions != null && typeof structuredOptions === 'object') {
      const {
        cause,
        ...causelessOptions
      } = structuredOptions;
      Object.assign(this, causelessOptions);
    }
  }
}
var _default = exports.default = ApiDOMStructuredError;