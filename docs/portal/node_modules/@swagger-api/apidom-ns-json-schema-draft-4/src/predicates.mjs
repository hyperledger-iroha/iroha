import { createPredicate } from '@swagger-api/apidom-core';
import JSONSchemaElement from "./elements/JSONSchema.mjs";
import JSONReferenceElement from "./elements/JSONReference.mjs";
import MediaElement from "./elements/Media.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
/**
 * @public
 */
export const isJSONSchemaElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof JSONSchemaElement || hasBasicElementProps(element) && isElementType('JSONSchemaDraft4', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isJSONReferenceElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof JSONReferenceElement || hasBasicElementProps(element) && isElementType('JSONReference', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isMediaElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof MediaElement || hasBasicElementProps(element) && isElementType('media', element) && primitiveEq('object', element);
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