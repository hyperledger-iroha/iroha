import { keyMap as keyMapBase } from '@swagger-api/apidom-core';
export { getNodeType } from '@swagger-api/apidom-ns-json-schema-2019-09';
/**
 * @public
 */
export const keyMap = {
  JSONSchema202012Element: ['content'],
  LinkDescriptionElement: ['content'],
  ...keyMapBase
};