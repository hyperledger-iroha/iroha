import { reduce } from 'ramda';
import { isPrimitiveElement, toValue } from '@swagger-api/apidom-core';
import { SchemaElement } from '@swagger-api/apidom-ns-openapi-3-1';
import * as url from "../../../util/url.mjs";
/**
 * @public
 */
export const resolveSchema$refField = (retrievalURI, schemaElement) => {
  if (typeof schemaElement.$ref === 'undefined') {
    return undefined;
  }
  const hash = url.getHash(toValue(schemaElement.$ref));
  const ancestorsSchemaIdentifiers = toValue(schemaElement.meta.get('ancestorsSchemaIdentifiers'));
  const $refBaseURI = reduce((acc, uri) => {
    return url.resolve(acc, url.sanitize(url.stripHash(uri)));
  }, retrievalURI, [...ancestorsSchemaIdentifiers, toValue(schemaElement.$ref)]);
  return `${$refBaseURI}${hash === '#' ? '' : hash}`;
};

/**
 * @public
 */
export const resolveSchema$idField = (retrievalURI, schemaElement) => {
  if (typeof schemaElement.$id === 'undefined') {
    return undefined;
  }
  const ancestorsSchemaIdentifiers = toValue(schemaElement.meta.get('ancestorsSchemaIdentifiers'));
  return reduce((acc, $id) => {
    return url.resolve(acc, url.sanitize(url.stripHash($id)));
  }, retrievalURI, ancestorsSchemaIdentifiers);
};

/**
 * Cached version of SchemaElement.refract.
 */
export const refractToSchemaElement = element => {
  if (refractToSchemaElement.cache.has(element)) {
    return refractToSchemaElement.cache.get(element);
  }
  const refracted = SchemaElement.refract(element);
  refractToSchemaElement.cache.set(element, refracted);
  return refracted;
};
refractToSchemaElement.cache = new WeakMap();

/**
 * @public
 */
export const maybeRefractToSchemaElement = element => {
  /**
   * Conditional version of refractToSchemaElement, that acts as an identity
   * function for all non-primitive Element instances.
   */
  if (isPrimitiveElement(element)) {
    return refractToSchemaElement(element);
  }
  return element;
};