"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
/**
 * @public
 */

/**
 * @public
 */
class ElementIdentityError extends _apidomError.ApiDOMStructuredError {
  value;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.value = structuredOptions.value;
    }
  }
}
var _default = exports.default = ElementIdentityError;