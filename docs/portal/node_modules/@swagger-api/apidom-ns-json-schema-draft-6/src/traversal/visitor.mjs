import { keyMap as keyMapBase } from '@swagger-api/apidom-core';
export { getNodeType } from '@swagger-api/apidom-ns-json-schema-draft-4';

/**
 * @public
 */
export const keyMap = {
  JSONSchemaDraft6Element: ['content'],
  JSONReferenceElement: ['content'],
  MediaElement: ['content'],
  LinkDescriptionElement: ['content'],
  ...keyMapBase
};