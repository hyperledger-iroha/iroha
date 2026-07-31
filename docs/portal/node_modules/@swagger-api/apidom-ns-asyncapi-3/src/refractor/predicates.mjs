import { startsWith } from 'ramda';
import { isObjectElement, isStringElement, toValue } from '@swagger-api/apidom-core';
export const isReferenceObject = node => {
  if (!node || typeof node !== 'object') return false;
  // ApiDOM value may expose $ref directly
  return typeof node.$ref === 'string' || typeof node.get === 'function' && node.get('$ref');
};
/**
 * @public
 */
export const isReferenceLikeElement = element => {
  return isObjectElement(element) && element.hasKey('$ref');
};

/**
 * @public
 */
export const isAsyncApiExtension = element => {
  return isStringElement(element.key) && startsWith('x-', toValue(element.key));
};

/**
 * @public
 */

/**
 * @public
 */
export const isMultiFormatSchemaLikeElement = element => {
  return isObjectElement(element) && element.hasKey('schemaFormat');
};
export default {
  isReferenceObject
};