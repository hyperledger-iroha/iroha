"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isServerVariableElement = exports.isServerElement = exports.isSecuritySchemeElement = exports.isSecurityRequirementElement = exports.isSchemaElement = exports.isResponsesElement = exports.isResponseElement = exports.isRequestBodyElement = exports.isReferenceElementExternal = exports.isReferenceElement = exports.isPathsElement = exports.isPathItemElementExternal = exports.isPathItemElement = exports.isParameterElement = exports.isOperationElement = exports.isOpenapiElement = exports.isOpenApi3_1Element = exports.isMediaTypeElement = exports.isLinkElement = exports.isLicenseElement = exports.isJsonSchemaDialectElement = exports.isInfoElement = exports.isHeaderElement = exports.isExternalDocumentationElement = exports.isExampleElement = exports.isContactElement = exports.isComponentsElement = exports.isCallbackElement = exports.isBooleanJsonSchemaElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _Callback = _interopRequireDefault(require("./elements/Callback.cjs"));
var _Components = _interopRequireDefault(require("./elements/Components.cjs"));
var _Contact = _interopRequireDefault(require("./elements/Contact.cjs"));
var _Example = _interopRequireDefault(require("./elements/Example.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("./elements/ExternalDocumentation.cjs"));
var _Header = _interopRequireDefault(require("./elements/Header.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _JsonSchemaDialect = _interopRequireDefault(require("./elements/JsonSchemaDialect.cjs"));
var _License = _interopRequireDefault(require("./elements/License.cjs"));
var _Link = _interopRequireDefault(require("./elements/Link.cjs"));
var _Openapi = _interopRequireDefault(require("./elements/Openapi.cjs"));
var _OpenApi = _interopRequireDefault(require("./elements/OpenApi3-1.cjs"));
var _Operation = _interopRequireDefault(require("./elements/Operation.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _PathItem = _interopRequireDefault(require("./elements/PathItem.cjs"));
var _Paths = _interopRequireDefault(require("./elements/Paths.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _RequestBody = _interopRequireDefault(require("./elements/RequestBody.cjs"));
var _Response = _interopRequireDefault(require("./elements/Response.cjs"));
var _Responses = _interopRequireDefault(require("./elements/Responses.cjs"));
var _Schema = _interopRequireDefault(require("./elements/Schema.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("./elements/SecurityRequirement.cjs"));
var _SecurityScheme = _interopRequireDefault(require("./elements/SecurityScheme.cjs"));
var _Server = _interopRequireDefault(require("./elements/Server.cjs"));
var _ServerVariable = _interopRequireDefault(require("./elements/ServerVariable.cjs"));
var _MediaType = _interopRequireDefault(require("./elements/MediaType.cjs"));
/**
 * @public
 */
const isCallbackElement = exports.isCallbackElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Callback.default || hasBasicElementProps(element) && isElementType('callback', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isComponentsElement = exports.isComponentsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Components.default || hasBasicElementProps(element) && isElementType('components', element) && primitiveEq('object', element);
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
const isJsonSchemaDialectElement = exports.isJsonSchemaDialectElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _JsonSchemaDialect.default || hasBasicElementProps(element) && isElementType('jsonSchemaDialect', element) && primitiveEq('string', element);
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
const isLinkElement = exports.isLinkElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Link.default || hasBasicElementProps(element) && isElementType('link', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isOpenapiElement = exports.isOpenapiElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Openapi.default || hasBasicElementProps(element) && isElementType('openapi', element) && primitiveEq('string', element);
});

/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
const isOpenApi3_1Element = exports.isOpenApi3_1Element = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _OpenApi.default || hasBasicElementProps(element) && isElementType('openApi3_1', element) && primitiveEq('object', element) && hasClass('api', element);
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
const isPathItemElement = exports.isPathItemElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _PathItem.default || hasBasicElementProps(element) && isElementType('pathItem', element) && primitiveEq('object', element);
});

/**
 * @deprecated
 * Determining whether a PathItemElement is external or internal is not possible by just looking
 * at value of the $ref fixed field. The value of the $ref field needs to be resolved in runtime
 * using the referring document as the Base URI.
 * @public
 */
const isPathItemElementExternal = element => {
  if (!isPathItemElement(element)) {
    return false;
  }
  if (!(0, _apidomCore.isStringElement)(element.$ref)) {
    return false;
  }
  const value = (0, _apidomCore.toValue)(element.$ref);
  return typeof value === 'string' && value.length > 0 && !value.startsWith('#');
};

/**
 * @public
 */
exports.isPathItemElementExternal = isPathItemElementExternal;
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
const isReferenceElement = exports.isReferenceElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Reference.default || hasBasicElementProps(element) && isElementType('reference', element) && primitiveEq('object', element);
});

/**
 * @deprecated
 * Determining whether a ReferenceElement is external or internal is not possible by just looking
 * at value of the $ref fixed field. The value of the $ref field needs to be resolved in runtime
 * using the referring document as the Base URI.
 * @public
 */
const isReferenceElementExternal = element => {
  if (!isReferenceElement(element)) {
    return false;
  }
  if (!(0, _apidomCore.isStringElement)(element.$ref)) {
    return false;
  }
  const value = (0, _apidomCore.toValue)(element.$ref);
  return typeof value === 'string' && value.length > 0 && !value.startsWith('#');
};

/**
 * @public
 */
exports.isReferenceElementExternal = isReferenceElementExternal;
const isRequestBodyElement = exports.isRequestBodyElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _RequestBody.default || hasBasicElementProps(element) && isElementType('requestBody', element) && primitiveEq('object', element);
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
const isBooleanJsonSchemaElement = element => {
  return (0, _apidomCore.isBooleanElement)(element) && element.classes.includes('boolean-json-schema');
};

/**
 * @public
 */
exports.isBooleanJsonSchemaElement = isBooleanJsonSchemaElement;
const isSecurityRequirementElement = exports.isSecurityRequirementElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SecurityRequirement.default || hasBasicElementProps(element) && isElementType('securityRequirement', element) && primitiveEq('object', element);
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
const isServerElement = exports.isServerElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Server.default || hasBasicElementProps(element) && isElementType('server', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isServerVariableElement = exports.isServerVariableElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ServerVariable.default || hasBasicElementProps(element) && isElementType('serverVariable', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isMediaTypeElement = exports.isMediaTypeElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _MediaType.default || hasBasicElementProps(element) && isElementType('mediaType', element) && primitiveEq('object', element);
});