import { isElement, isStringElement, isArrayElement, isObjectElement, isMemberElement, createNamespace, includesClasses, hasElementSourceMap } from '@swagger-api/apidom-core';
import * as openApi3_0Predicates from "../predicates.mjs";
import openApi3_0Namespace from "../namespace.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
const createToolbox = () => {
  const namespace = createNamespace(openApi3_0Namespace);
  const predicates = {
    ...openApi3_0Predicates,
    isElement,
    isStringElement,
    isArrayElement,
    isObjectElement,
    isMemberElement,
    includesClasses,
    hasElementSourceMap
  };
  return {
    predicates,
    namespace
  };
};
export default createToolbox;