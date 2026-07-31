"use strict";

exports.__esModule = true;
exports.isReferenceObject = exports.isReferenceLikeElement = exports.isMultiFormatSchemaLikeElement = exports.isAsyncApiExtension = exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
const isReferenceObject = node => {
  if (!node || typeof node !== 'object') return false;
  // ApiDOM value may expose $ref directly
  return typeof node.$ref === 'string' || typeof node.get === 'function' && node.get('$ref');
};
exports.isReferenceObject = isReferenceObject;
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

/**
 * @public
 */
exports.isAsyncApiExtension = isAsyncApiExtension;
/**
 * @public
 */
const isMultiFormatSchemaLikeElement = element => {
  return (0, _apidomCore.isObjectElement)(element) && element.hasKey('schemaFormat');
};
exports.isMultiFormatSchemaLikeElement = isMultiFormatSchemaLikeElement;
var _default = exports.default = {
  isReferenceObject
};