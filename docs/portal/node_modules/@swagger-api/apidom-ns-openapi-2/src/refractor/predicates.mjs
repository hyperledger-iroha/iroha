import { startsWith } from 'ramda';
import { isStringElement, toValue, isObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */

/**
 * @public
 */
export const isSwaggerExtension = element => {
  return isStringElement(element.key) && startsWith('x-', toValue(element.key));
};

/**
 * @public
 */
export const isReferenceLikeElement = element => {
  return isObjectElement(element) && element.hasKey('$ref');
};