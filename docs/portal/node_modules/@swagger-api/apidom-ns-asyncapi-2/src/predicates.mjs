import { createPredicate, isBooleanElement } from '@swagger-api/apidom-core';
import AsyncApi2Element from "./elements/AsyncApi2.mjs";
import AsyncApiVersionElement from "./elements/AsyncApiVersion.mjs";
import ChannelBindingsElement from "./elements/ChannelBindings.mjs";
import ChannelItemElement from "./elements/ChannelItem.mjs";
import ChannelsElement from "./elements/Channels.mjs";
import ComponentsElement from "./elements/Components.mjs";
import ContactElement from "./elements/Contact.mjs";
import IdentifierElement from "./elements/Identifier.mjs";
import InfoElement from "./elements/Info.mjs";
import LicenseElement from "./elements/License.mjs";
import OperationElement from "./elements/Operation.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import ParametersElement from "./elements/Parameters.mjs";
import ReferenceElement from "./elements/Reference.mjs";
import SchemaElement from "./elements/Schema.mjs";
import SecurityRequirementElement from "./elements/SecurityRequirement.mjs";
import ServerElement from "./elements/Server.mjs";
import ServerBindingsElement from "./elements/ServerBindings.mjs";
import ServersElement from "./elements/Servers.mjs";
import ServerVariableElement from "./elements/ServerVariable.mjs";
/**
 * @public
 */
export const isAsyncApi2Element = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof AsyncApi2Element && element.constructor === AsyncApi2Element || hasBasicElementProps(element) && isElementType('asyncApi2', element) && primitiveEq('object', element) && hasClass('api', element);
});

/**
 * @public
 */
export const isAsyncApiVersionElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof AsyncApiVersionElement || hasBasicElementProps(element) && isElementType('asyncApiVersion', element) && primitiveEq('string', element);
});

/**
 * @public
 */
export const isChannelBindingsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ChannelBindingsElement || hasBasicElementProps(element) && isElementType('channelBindings', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isChannelItemElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ChannelItemElement || hasBasicElementProps(element) && isElementType('channelItem', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isChannelsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ChannelsElement || hasBasicElementProps(element) && isElementType('channels', element) && primitiveEq('object', element);
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
export const isIdentifierElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof IdentifierElement || hasBasicElementProps(element) && isElementType('identifier', element) && primitiveEq('string', element);
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
export const isParametersElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ParametersElement || hasBasicElementProps(element) && isElementType('parameters', element) && primitiveEq('object', element);
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
export const isServerBindingsElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ServerBindingsElement || hasBasicElementProps(element) && isElementType('serverBindings', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isServersElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ServersElement || hasBasicElementProps(element) && isElementType('servers', element) && primitiveEq('object', element);
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