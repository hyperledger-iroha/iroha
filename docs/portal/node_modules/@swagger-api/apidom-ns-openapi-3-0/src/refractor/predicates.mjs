import { startsWith } from 'ramda';
import { isStringElement, isObjectElement, toValue } from '@swagger-api/apidom-core';

/**
 * @public
 */

/**
 * @public
 */
export const isReferenceLikeElement = element => {
  return isObjectElement(element) && element.hasKey('$ref');
};

/**
 * @public
 */
export const isServerLikeElement = isObjectElement;

/**
 * @public
 */
export const isTagLikeElement = isObjectElement;

/**
 * @public
 */
export const isOpenApiExtension = element => {
  // @ts-ignore
  return isStringElement(element.key) && startsWith('x-', toValue(element.key));
};