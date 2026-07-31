import { parse as jsonPointerParse, URIFragmentIdentifier } from '@swaggerexpert/json-pointer';

/**
 * @public
 * @deprecated
 */
const parse = (pointer, options) => {
  const {
    tree
  } = jsonPointerParse(URIFragmentIdentifier.from(pointer), options);
  return tree;
};

/**
 * Returns the hash (URL fragment), of the given path.
 * If there is no hash, then the root hash ("#") is returned.
 */
const getHash = uri => {
  const hashIndex = uri.indexOf('#');
  if (hashIndex !== -1) {
    return uri.substring(hashIndex);
  }
  return '#';
};

/**
 * @public
 * @deprecated
 */
export const uriToPointer = uri => {
  const hash = getHash(uri);
  return URIFragmentIdentifier.from(hash);
};
export default parse;