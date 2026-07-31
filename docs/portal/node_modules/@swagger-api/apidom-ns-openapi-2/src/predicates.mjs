import { createPredicate } from '@swagger-api/apidom-core';
import SwaggerElement from "./elements/Swagger.mjs";
import SwaggerVersionElement from "./elements/SwaggerVersion.mjs";
import InfoElement from "./elements/Info.mjs";
import ContactElement from "./elements/Contact.mjs";
import LicenseElement from "./elements/License.mjs";
import PathsElement from "./elements/Paths.mjs";
import PathItemElement from "./elements/PathItem.mjs";
import OperationElement from "./elements/Operation.mjs";
import ExternalDocumentationElement from "./elements/ExternalDocumentation.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import ItemsElement from "./elements/Items.mjs";
import ExampleElement from "./elements/Example.mjs";
import ResponsesElement from "./elements/Responses.mjs";
import ResponseElement from "./elements/Response.mjs";
import HeadersElement from "./elements/Headers.mjs";
import HeaderElement from "./elements/Header.mjs";
import TagElement from "./elements/Tag.mjs";
import ReferenceElement from "./elements/Reference.mjs";
import SchemaElement from "./elements/Schema.mjs";
import XmlElement from "./elements/Xml.mjs";
import DefinitionsElement from "./elements/Definitions.mjs";
import ParametersDefinitionsElement from "./elements/ParametersDefinitions.mjs";
import ResponsesDefinitionsElement from "./elements/ResponsesDefinitions.mjs";
import SecurityDefinitionsElement from "./elements/SecurityDefinitions.mjs";
import SecuritySchemeElement from "./elements/SecurityScheme.mjs";
import SecurityRequirementElement from "./elements/SecurityRequirement.mjs";
import ScopesElement from "./elements/Scopes.mjs";
/**
 * @public
 */
export const isSwaggerElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof SwaggerElement || hasBasicElementProps(element) && isElementType('swagger', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isSwaggerVersionElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof SwaggerVersionElement || hasBasicElementProps(element) && isElementType('swaggerVersion', element) && primitiveEq('string', element);
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
export const isItemsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ItemsElement || hasBasicElementProps(element) && isElementType('items', element) && primitiveEq('object', element);
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
export const isHeadersElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof HeadersElement || hasBasicElementProps(element) && isElementType('headers', element) && primitiveEq('object', element);
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
export const isTagElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof TagElement || hasBasicElementProps(element) && isElementType('tag', element) && primitiveEq('object', element);
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
export const isXmlElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof XmlElement || hasBasicElementProps(element) && isElementType('xml', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isResponsesDefinitionsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ResponsesDefinitionsElement || hasBasicElementProps(element) && isElementType('responsesDefinitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isSecurityDefinitionsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof SecurityDefinitionsElement || hasBasicElementProps(element) && isElementType('securityDefinitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isDefinitionsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof DefinitionsElement || hasBasicElementProps(element) && isElementType('definitions', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isParametersDefinitionsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ParametersDefinitionsElement || hasBasicElementProps(element) && isElementType('parametersDefinitions', element) && primitiveEq('object', element);
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
export const isScopesElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ScopesElement || hasBasicElementProps(element) && isElementType('scopes', element) && primitiveEq('object', element);
});

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