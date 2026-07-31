import { keyMap as keyMapBase, isElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
export const getNodeType = element => {
  if (!isElement(element)) {
    return undefined;
  }
  return `${element.element.charAt(0).toUpperCase() + element.element.slice(1)}Element`;
};

/**
 * @public
 */
export const keyMap = {
  JSONSchemaDraft4Element: ['content'],
  JSONReferenceElement: ['content'],
  MediaElement: ['content'],
  LinkDescriptionElement: ['content'],
  ...keyMapBase
};