import { isObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */

// eslint-disable-next-line import/prefer-default-export
/**
 * @public
 */
export const isJSONReferenceLikeElement = element => {
  return isObjectElement(element) && element.hasKey('$ref');
};