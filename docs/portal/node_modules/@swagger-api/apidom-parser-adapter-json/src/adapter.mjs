import { createNamespace } from '@swagger-api/apidom-core';
export { default as mediaTypes } from "./media-types.mjs";
/**
 * @public
 */
export const namespace = createNamespace();

/**
 * @public
 */
export const detectionRegExp =
// eslint-disable-next-line no-control-regex
/(?<true>^\s*true\s*$)|(?<false>^\s*false\s*$)|(?<null>^\s*null\s*$)|(?<number>^\s*\d+\s*$)|(?<object>^\s*{\s*)|(?<array>^\s*\[\s*)|(?<string>^\s*"(((?=\\)\\(["\\/bfnrt]|u[0-9a-fA-F]{4}))|[^"\\\x00-\x1F\x7F])*"\s*$)/;