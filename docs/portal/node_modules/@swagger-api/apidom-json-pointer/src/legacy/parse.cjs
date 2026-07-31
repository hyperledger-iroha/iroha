"use strict";

exports.__esModule = true;
exports.uriToPointer = exports.default = void 0;
var _jsonPointer = require("@swaggerexpert/json-pointer");
/**
 * @public
 * @deprecated
 */
const parse = (pointer, options) => {
  const {
    tree
  } = (0, _jsonPointer.parse)(_jsonPointer.URIFragmentIdentifier.from(pointer), options);
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
const uriToPointer = uri => {
  const hash = getHash(uri);
  return _jsonPointer.URIFragmentIdentifier.from(hash);
};
exports.uriToPointer = uriToPointer;
var _default = exports.default = parse;