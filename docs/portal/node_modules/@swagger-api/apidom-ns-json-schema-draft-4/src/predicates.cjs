"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isMediaElement = exports.isLinkDescriptionElement = exports.isJSONSchemaElement = exports.isJSONReferenceElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _JSONReference = _interopRequireDefault(require("./elements/JSONReference.cjs"));
var _Media = _interopRequireDefault(require("./elements/Media.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
/**
 * @public
 */
const isJSONSchemaElement = exports.isJSONSchemaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _JSONSchema.default || hasBasicElementProps(element) && isElementType('JSONSchemaDraft4', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isJSONReferenceElement = exports.isJSONReferenceElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _JSONReference.default || hasBasicElementProps(element) && isElementType('JSONReference', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isMediaElement = exports.isMediaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Media.default || hasBasicElementProps(element) && isElementType('media', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isLinkDescriptionElement = exports.isLinkDescriptionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _LinkDescription.default || hasBasicElementProps(element) && isElementType('linkDescription', element) && primitiveEq('object', element);
});