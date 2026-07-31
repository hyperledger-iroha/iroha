import { createPredicate, isBooleanElement } from '@swagger-api/apidom-core';
import CallbackElement from "./elements/Callback.mjs";
import ComponentsElement from "./elements/Components.mjs";
import ContactElement from "./elements/Contact.mjs";
import ExampleElement from "./elements/Example.mjs";
import ExternalDocumentationElement from "./elements/ExternalDocumentation.mjs";
import HeaderElement from "./elements/Header.mjs";
import InfoElement from "./elements/Info.mjs";
import LicenseElement from "./elements/License.mjs";
import LinkElement from "./elements/Link.mjs";
import OpenapiElement from "./elements/Openapi.mjs";
import OpenApi3_0Element from "./elements/OpenApi3-0.mjs";
import OperationElement from "./elements/Operation.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import PathItemElement from "./elements/PathItem.mjs";
import PathsElement from "./elements/Paths.mjs";
import ReferenceElement from "./elements/Reference.mjs";
import RequestBodyElement from "./elements/RequestBody.mjs";
import ResponseElement from "./elements/Response.mjs";
import ResponsesElement from "./elements/Responses.mjs";
import SchemaElement from "./elements/Schema.mjs";
import SecurityRequirementElement from "./elements/SecurityRequirement.mjs";
import SecuritySchemeElement from "./elements/SecurityScheme.mjs";
import ServerElement from "./elements/Server.mjs";
import ServerVariableElement from "./elements/ServerVariable.mjs";
import MediaTypeElement from "./elements/MediaType.mjs";
import DiscriminatorElement from "./elements/Discriminator.mjs"; // NCE types
import ServersElement from "./elements/nces/Servers.mjs";
/**
 * @public
 */
export const isCallbackElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof CallbackElement || hasBasicElementProps(element) && isElementType('callback', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isComponentsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ComponentsElement || hasBasicElementProps(element) && isElementType('components', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isContactElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ContactElement || hasBasicElementProps(element) && isElementType('contact', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isExampleElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ExampleElement || hasBasicElementProps(element) && isElementType('example', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isExternalDocumentationElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ExternalDocumentationElement || hasBasicElementProps(element) && isElementType('externalDocumentation', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isHeaderElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof HeaderElement || hasBasicElementProps(element) && isElementType('header', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isInfoElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof InfoElement || hasBasicElementProps(element) && isElementType('info', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isLicenseElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof LicenseElement || hasBasicElementProps(element) && isElementType('license', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isLinkElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof LinkElement || hasBasicElementProps(element) && isElementType('link', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isOpenapiElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof OpenapiElement || hasBasicElementProps(element) && isElementType('openapi', element) && primitiveEq('string', element);
});

/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
export const isOpenApi3_0Element = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof OpenApi3_0Element || hasBasicElementProps(element) && isElementType('openApi3_0', element) && primitiveEq('object', element) && hasClass('api', element);
});

/**
 * @public
 */
export const isOperationElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof OperationElement || hasBasicElementProps(element) && isElementType('operation', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isParameterElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ParameterElement || hasBasicElementProps(element) && isElementType('parameter', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isPathItemElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof PathItemElement || hasBasicElementProps(element) && isElementType('pathItem', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isPathsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof PathsElement || hasBasicElementProps(element) && isElementType('paths', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isReferenceElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ReferenceElement || hasBasicElementProps(element) && isElementType('reference', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isRequestBodyElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof RequestBodyElement || hasBasicElementProps(element) && isElementType('requestBody', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isResponseElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ResponseElement || hasBasicElementProps(element) && isElementType('response', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isResponsesElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ResponsesElement || hasBasicElementProps(element) && isElementType('responses', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isSchemaElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof SchemaElement || hasBasicElementProps(element) && isElementType('schema', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isBooleanJsonSchemaElement = element => {
  return isBooleanElement(element) && element.classes.includes('boolean-json-schema');
};

/**
 * @public
 */
export const isSecurityRequirementElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof SecurityRequirementElement || hasBasicElementProps(element) && isElementType('securityRequirement', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isSecuritySchemeElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof SecuritySchemeElement || hasBasicElementProps(element) && isElementType('securityScheme', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isServerElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ServerElement || hasBasicElementProps(element) && isElementType('server', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isServerVariableElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ServerVariableElement || hasBasicElementProps(element) && isElementType('serverVariable', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isMediaTypeElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof MediaTypeElement || hasBasicElementProps(element) && isElementType('mediaType', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isServersElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof ServersElement || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('servers', element);
});

/**
 * @public
 */
export const isDiscriminatorElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof DiscriminatorElement || hasBasicElementProps(element) && isElementType('discriminator', element) && primitiveEq('object', element);
});