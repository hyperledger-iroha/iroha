import { keyMap as keyMapBase } from '@swagger-api/apidom-core';
export { getNodeType } from '@swagger-api/apidom-ns-json-schema-draft-7';

/**
 * @public
 */
export const keyMap = {
  JSONSchema201909Element: ['content'],
  LinkDescriptionElement: ['content'],
  ...keyMapBase
};