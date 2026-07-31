import { isUndefined } from 'ramda-adjunct';
import { filter } from '@swagger-api/apidom-core';
import { isSchemaElement } from '@swagger-api/apidom-ns-openapi-3-1';
import { URIFragmentIdentifier, evaluate as jsonPointerEvaluate } from '@swagger-api/apidom-json-pointer/modern';
import * as url from "../../../../util/url.mjs";
import EvaluationJsonSchemaUriError from "../../../../errors/EvaluationJsonSchemaUriError.mjs";
import { isAnchor, uriToAnchor, evaluate as $anchorEvaluate } from "./$anchor.mjs";
import { resolveSchema$idField } from "../util.mjs";
/**
 * Evaluates JSON Schema $ref containing unknown URI against ApiDOM fragment.
 * @public
 */
export const evaluate = (uri, element) => {
  const {
    cache
  } = evaluate;
  const uriStrippedHash = url.stripHash(uri);
  const isSchemaElementWith$id = e => isSchemaElement(e) && typeof e.$id !== 'undefined';

  // warm the cache
  if (!cache.has(element)) {
    const schemaObjectElements = filter(isSchemaElementWith$id, element);
    cache.set(element, Array.from(schemaObjectElements));
  }

  // search for the matching schema
  const result = cache.get(element).find(e => {
    const $idBaseURI = resolveSchema$idField(uriStrippedHash, e);
    return $idBaseURI === uriStrippedHash;
  });
  if (isUndefined(result)) {
    throw new EvaluationJsonSchemaUriError(`Evaluation failed on URI: "${uri}"`);
  }
  if (isAnchor(uriToAnchor(uri))) {
    // we're dealing with JSON Schema $anchor here
    return $anchorEvaluate(uriToAnchor(uri), result);
  }
  return jsonPointerEvaluate(result, URIFragmentIdentifier.fromURIReference(uri));
};
evaluate.cache = new WeakMap();
export { EvaluationJsonSchemaUriError };
export { default as JsonSchemaUriError } from "../../../../errors/JsonSchemaUriError.mjs";