"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isXmlElement = exports.isTagElement = exports.isSwaggerVersionElement = exports.isSwaggerElement = exports.isSecuritySchemeElement = exports.isSecurityRequirementElement = exports.isSecurityDefinitionsElement = exports.isScopesElement = exports.isSchemaElement = exports.isResponsesElement = exports.isResponsesDefinitionsElement = exports.isResponseElement = exports.isReferenceElement = exports.isPathsElement = exports.isPathItemElement = exports.isParametersDefinitionsElement = exports.isParameterElement = exports.isOperationElement = exports.isLicenseElement = exports.isItemsElement = exports.isInfoElement = exports.isHeadersElement = exports.isHeaderElement = exports.isExternalDocumentationElement = exports.isExampleElement = exports.isDefinitionsElement = exports.isContactElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _Swagger = _interopRequireDefault(require("./elements/Swagger.cjs"));
var _SwaggerVersion = _interopRequireDefault(require("./elements/SwaggerVersion.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _Contact = _interopRequireDefault(require("./elements/Contact.cjs"));
var _License = _interopRequireDefault(require("./elements/License.cjs"));
var _Paths = _interopRequireDefault(require("./elements/Paths.cjs"));
var _PathItem = _interopRequireDefault(require("./elements/PathItem.cjs"));
var _Operation = _interopRequireDefault(require("./elements/Operation.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("./elements/ExternalDocumentation.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _Items = _interopRequireDefault(require("./elements/Items.cjs"));
var _Example = _interopRequireDefault(require("./elements/Example.cjs"));
var _Responses = _interopRequireDefault(require("./elements/Responses.cjs"));
var _Response = _interopRequireDefault(require("./elements/Response.cjs"));
var _Headers = _interopRequireDefault(require("./elements/Headers.cjs"));
var _Header = _interopRequireDefault(require("./elements/Header.cjs"));
var _Tag = _interopRequireDefault(require("./elements/Tag.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _Schema = _interopRequireDefault(require("./elements/Schema.cjs"));
var _Xml = _interopRequireDefault(require("./elements/Xml.cjs"));
var _Definitions = _interopRequireDefault(require("./elements/Definitions.cjs"));
var _ParametersDefinitions = _interopRequireDefault(require("./elements/ParametersDefinitions.cjs"));
var _ResponsesDefinitions = _interopRequireDefault(require("./elements/ResponsesDefinitions.cjs"));
var _SecurityDefinitions = _interopRequireDefault(require("./elements/SecurityDefinitions.cjs"));
var _SecurityScheme = _interopRequireDefault(require("./elements/SecurityScheme.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("./elements/SecurityRequirement.cjs"));
var _Scopes = _interopRequireDefault(require("./elements/Scopes.cjs"));
/**
 * @public
 */
const isSwaggerElement = exports.isSwaggerElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Swagger.default || hasBasicElementProps(element) && isElementType('swagger', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSwaggerVersionElement = exports.isSwaggerVersionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SwaggerVersion.default || hasBasicElementProps(element) && isElementType('swaggerVersion', element) && primitiveEq('string', element);
});

/**
 * @public
 */
const isInfoElement = exports.isInfoElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Info.default || hasBasicElementProps(element) && isElementType('info', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isLicenseElement = exports.isLicenseElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _License.default || hasBasicElementProps(element) && isElementType('license', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isContactElement = exports.isContactElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Contact.default || hasBasicElementProps(element) && isElementType('contact', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isPathsElement = exports.isPathsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Paths.default || hasBasicElementProps(element) && isElementType('paths', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isPathItemElement = exports.isPathItemElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _PathItem.default || hasBasicElementProps(element) && isElementType('pathItem', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isOperationElement = exports.isOperationElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Operation.default || hasBasicElementProps(element) && isElementType('operation', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isExternalDocumentationElement = exports.isExternalDocumentationElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ExternalDocumentation.default || hasBasicElementProps(element) && isElementType('externalDocumentation', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isParameterElement = exports.isParameterElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Parameter.default || hasBasicElementProps(element) && isElementType('parameter', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isItemsElement = exports.isItemsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Items.default || hasBasicElementProps(element) && isElementType('items', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isResponsesElement = exports.isResponsesElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Responses.default || hasBasicElementProps(element) && isElementType('responses', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isResponseElement = exports.isResponseElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Response.default || hasBasicElementProps(element) && isElementType('response', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isHeadersElement = exports.isHeadersElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Headers.default || hasBasicElementProps(element) && isElementType('headers', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isExampleElement = exports.isExampleElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Example.default || hasBasicElementProps(element) && isElementType('example', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isHeaderElement = exports.isHeaderElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Header.default || hasBasicElementProps(element) && isElementType('header', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isTagElement = exports.isTagElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Tag.default || hasBasicElementProps(element) && isElementType('tag', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isReferenceElement = exports.isReferenceElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Reference.default || hasBasicElementProps(element) && isElementType('reference', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSchemaElement = exports.isSchemaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Schema.default || hasBasicElementProps(element) && isElementType('schema', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isXmlElement = exports.isXmlElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Xml.default || hasBasicElementProps(element) && isElementType('xml', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isResponsesDefinitionsElement = exports.isResponsesDefinitionsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ResponsesDefinitions.default || hasBasicElementProps(element) && isElementType('responsesDefinitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSecurityDefinitionsElement = exports.isSecurityDefinitionsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SecurityDefinitions.default || hasBasicElementProps(element) && isElementType('securityDefinitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isDefinitionsElement = exports.isDefinitionsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Definitions.default || hasBasicElementProps(element) && isElementType('definitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isParametersDefinitionsElement = exports.isParametersDefinitionsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ParametersDefinitions.default || hasBasicElementProps(element) && isElementType('parametersDefinitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSecuritySchemeElement = exports.isSecuritySchemeElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SecurityScheme.default || hasBasicElementProps(element) && isElementType('securityScheme', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isScopesElement = exports.isScopesElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Scopes.default || hasBasicElementProps(element) && isElementType('scopes', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSecurityRequirementElement = exports.isSecurityRequirementElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SecurityRequirement.default || hasBasicElementProps(element) && isElementType('securityRequirement', element) && primitiveEq('object', element);
});