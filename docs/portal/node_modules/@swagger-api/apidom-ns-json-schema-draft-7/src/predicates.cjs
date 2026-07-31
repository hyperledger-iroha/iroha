"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isLinkDescriptionElement = exports.isJSONSchemaElement = exports.isJSONReferenceElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-6");
exports.isJSONReferenceElement = _apidomNsJsonSchemaDraft.isJSONReferenceElement;
/**
 * @public
 */
const isJSONSchemaElement = exports.isJSONSchemaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _JSONSchema.default || hasBasicElementProps(element) && isElementType('JSONSchemaDraft7', element) && primitiveEq('object', element);
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