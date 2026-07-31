import { JSONPointerEvaluateError, URIFragmentIdentifier } from '@swaggerexpert/json-pointer';
import { evaluate as baseEvaluate } from '@swaggerexpert/json-pointer/evaluate/realms/apidom';
import { cloneDeep } from '@swagger-api/apidom-core';
import EvaluationJsonPointerError from "./errors/EvaluationJsonPointerError.mjs";
/**
 * Evaluates JSON Pointer against ApiDOM fragment.
 * @public
 * @deprecated
 */
const evaluate = (pointer, element) => {
  try {
    return baseEvaluate(element, URIFragmentIdentifier.from(pointer));
  } catch (error) {
    if (error instanceof JSONPointerEvaluateError) {
      throw new EvaluationJsonPointerError(error.message, {
        pointer,
        tokens: error.referenceTokens,
        failedToken: error.referenceToken,
        failedTokenPosition: error.referenceTokenPosition,
        element: cloneDeep(element),
        cause: error
      });
    }
    throw new EvaluationJsonPointerError(error.message, {
      pointer,
      element: cloneDeep(element),
      cause: error
    });
  }
};
export default evaluate;