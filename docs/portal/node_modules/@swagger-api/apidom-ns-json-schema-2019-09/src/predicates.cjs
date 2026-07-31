"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isLinkDescriptionElement = exports.isJSONSchemaElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
/**
 * @public
 */
const isJSONSchemaElement = exports.isJSONSchemaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _JSONSchema.default || hasBasicElementProps(element) && isElementType('JSONSchema201909', element) && primitiveEq('object', element);
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