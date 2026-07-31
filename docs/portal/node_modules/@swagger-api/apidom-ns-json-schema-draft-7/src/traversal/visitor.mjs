import { keyMap as keyMapBase } from '@swagger-api/apidom-core';
export { getNodeType } from '@swagger-api/apidom-ns-json-schema-draft-6';

/**
 * @public
 */
export const keyMap = {
  JSONSchemaDraft7Element: ['content'],
  JSONReferenceElement: ['content'],
  LinkDescriptionElement: ['content'],
  ...keyMapBase
};