import { createPredicate } from '@swagger-api/apidom-core';
import JSONSchemaElement from "./elements/JSONSchema.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
export { isJSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-6';

/**
 * @public
 */
export const isJSONSchemaElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof JSONSchemaElement || hasBasicElementProps(element) && isElementType('JSONSchemaDraft7', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isLinkDescriptionElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof LinkDescriptionElement || hasBasicElementProps(element) && isElementType('linkDescription', element) && primitiveEq('object', element);
});