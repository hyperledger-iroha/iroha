"use strict";

exports.__esModule = true;
exports.isSwaggerExtension = exports.isReferenceLikeElement = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
const isSwaggerExtension = element => {
  return (0, _apidomCore.isStringElement)(element.key) && (0, _ramda.startsWith)('x-', (0, _apidomCore.toValue)(element.key));
};

/**
 * @public
 */
exports.isSwaggerExtension = isSwaggerExtension;
const isReferenceLikeElement = element => {
  return (0, _apidomCore.isObjectElement)(element) && element.hasKey('$ref');
};
exports.isReferenceLikeElement = isReferenceLikeElement;