"use strict";

exports.__esModule = true;
exports.isReferenceLikeElement = exports.isAsyncApiExtension = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
const isReferenceLikeElement = element => {
  return (0, _apidomCore.isObjectElement)(element) && element.hasKey('$ref');
};

/**
 * @public
 */
exports.isReferenceLikeElement = isReferenceLikeElement;
const isAsyncApiExtension = element => {
  return (0, _apidomCore.isStringElement)(element.key) && (0, _ramda.startsWith)('x-', (0, _apidomCore.toValue)(element.key));
};
exports.isAsyncApiExtension = isAsyncApiExtension;