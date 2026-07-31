"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.resolveSchema$refField = exports.resolveSchema$idField = exports.refractToSchemaElement = exports.maybeRefractToSchemaElement = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
var url = _interopRequireWildcard(require("../../../util/url.cjs"));
/**
 * @public
 */
const resolveSchema$refField = (retrievalURI, schemaElement) => {
  if (typeof schemaElement.$ref === 'undefined') {
    return undefined;
  }
  const hash = url.getHash((0, _apidomCore.toValue)(schemaElement.$ref));
  const ancestorsSchemaIdentifiers = (0, _apidomCore.toValue)(schemaElement.meta.get('ancestorsSchemaIdentifiers'));
  const $refBaseURI = (0, _ramda.reduce)((acc, uri) => {
    return url.resolve(acc, url.sanitize(url.stripHash(uri)));
  }, retrievalURI, [...ancestorsSchemaIdentifiers, (0, _apidomCore.toValue)(schemaElement.$ref)]);
  return `${$refBaseURI}${hash === '#' ? '' : hash}`;
};

/**
 * @public
 */
exports.resolveSchema$refField = resolveSchema$refField;
const resolveSchema$idField = (retrievalURI, schemaElement) => {
  if (typeof schemaElement.$id === 'undefined') {
    return undefined;
  }
  const ancestorsSchemaIdentifiers = (0, _apidomCore.toValue)(schemaElement.meta.get('ancestorsSchemaIdentifiers'));
  return (0, _ramda.reduce)((acc, $id) => {
    return url.resolve(acc, url.sanitize(url.stripHash($id)));
  }, retrievalURI, ancestorsSchemaIdentifiers);
};

/**
 * Cached version of SchemaElement.refract.
 */
exports.resolveSchema$idField = resolveSchema$idField;
const refractToSchemaElement = element => {
  if (refractToSchemaElement.cache.has(element)) {
    return refractToSchemaElement.cache.get(element);
  }
  const refracted = _apidomNsOpenapi.SchemaElement.refract(element);
  refractToSchemaElement.cache.set(element, refracted);
  return refracted;
};
exports.refractToSchemaElement = refractToSchemaElement;
refractToSchemaElement.cache = new WeakMap();

/**
 * @public
 */
const maybeRefractToSchemaElement = element => {
  /**
   * Conditional version of refractToSchemaElement, that acts as an identity
   * function for all non-primitive Element instances.
   */
  if ((0, _apidomCore.isPrimitiveElement)(element)) {
    return refractToSchemaElement(element);
  }
  return element;
};
exports.maybeRefractToSchemaElement = maybeRefractToSchemaElement;