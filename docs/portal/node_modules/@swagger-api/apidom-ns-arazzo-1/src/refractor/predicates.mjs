import { startsWith } from 'ramda';
import { isStringElement, toValue, isObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
export const isArazzoSpecificationExtension = element => {
  return isStringElement(element.key) && startsWith('x-', toValue(element.key));
};
export const isReferenceLikeElement = element => {
  return isObjectElement(element) && element.hasKey('$ref');
};