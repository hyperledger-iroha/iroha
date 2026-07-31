import { trimCharsStart, isUndefined } from 'ramda-adjunct';
import { find, toValue } from '@swagger-api/apidom-core';
import { isSchemaElement } from '@swagger-api/apidom-ns-openapi-3-1';
import { getHash } from "../../../../util/url.mjs";
import EvaluationJsonSchema$anchorError from "../../../../errors/EvaluationJsonSchema$anchorError.mjs";
import InvalidJsonSchema$anchorError from "../../../../errors/InvalidJsonSchema$anchorError.mjs";
/**
 * @public
 */
export const isAnchor = uri => {
  /**
   *  MUST start with a letter ([A-Za-z]) or underscore ("_"), followed by any number of letters,
   *  digits ([0-9]), hyphens ("-"), underscores ("_"), and periods (".").
   *
   *  https://json-schema.org/draft/2020-12/json-schema-core.html#rfc.section.8.2.2
   */
  return /^[A-Za-z_][A-Za-z_0-9.-]*$/.test(uri);
};

/**
 * @public
 */
export const uriToAnchor = uri => {
  const hash = getHash(uri);
  return trimCharsStart('#', hash);
};

/**
 * @public
 */
export const parse = anchor => {
  if (!isAnchor(anchor)) {
    throw new InvalidJsonSchema$anchorError(anchor);
  }
  return anchor;
};

/**
 * Evaluates JSON Schema $anchor against ApiDOM fragment.
 * @public
 */
export const evaluate = (anchor, element) => {
  const token = parse(anchor);

  // @ts-ignore
  const result = find(e => isSchemaElement(e) && toValue(e.$anchor) === token, element);
  if (isUndefined(result)) {
    throw new EvaluationJsonSchema$anchorError(`Evaluation failed on token: "${token}"`);
  }

  // @ts-ignore
  return result;
};
export { EvaluationJsonSchema$anchorError, InvalidJsonSchema$anchorError };
export { default as JsonSchema$anchorError } from "../../../../errors/JsonSchema$anchorError.mjs";