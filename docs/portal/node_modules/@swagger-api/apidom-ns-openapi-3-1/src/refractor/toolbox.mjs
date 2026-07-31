import { isElement, isStringElement, isArrayElement, isObjectElement, isMemberElement, toValue, createNamespace, includesClasses, hasElementSourceMap } from '@swagger-api/apidom-core';
import { compile as compileJSONPointerTokens } from '@swagger-api/apidom-json-pointer/modern';
import { isServersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import * as openApi3_1Predicates from "../predicates.mjs";
import openApi3_1Namespace from "../namespace.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
/**
 * Translates visitor ancestor lineage to a JSON Pointer tokens.
 * Ancestor lineage is constructed of following visitor method arguments:
 *
 *  - ancestors
 *  - parent
 *  - element
 * @public
 */
export const ancestorLineageToJSONPointer = elementPath => {
  const jsonPointerTokens = elementPath.reduce((path, element, index) => {
    if (isMemberElement(element)) {
      const token = String(toValue(element.key));
      path.push(token);
    } else if (isArrayElement(elementPath[index - 2])) {
      const token = String(elementPath[index - 2].content.indexOf(element));
      path.push(token);
    }
    return path;
  }, []);
  return compileJSONPointerTokens(jsonPointerTokens);
};

/**
 * @public
 */
const createToolbox = () => {
  const namespace = createNamespace(openApi3_1Namespace);
  const predicates = {
    ...openApi3_1Predicates,
    isElement,
    isStringElement,
    isArrayElement,
    isObjectElement,
    isMemberElement,
    isServersElement,
    includesClasses,
    hasElementSourceMap
  };
  return {
    predicates,
    ancestorLineageToJSONPointer,
    namespace
  };
};
export default createToolbox;