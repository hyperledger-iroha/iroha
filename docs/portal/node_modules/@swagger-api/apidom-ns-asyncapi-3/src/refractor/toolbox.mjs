import { createNamespace, isStringElement } from '@swagger-api/apidom-core';
import * as asyncApi3Predicates from "../predicates.mjs";
import asyncApi3Namespace from "../namespace.mjs";
const createToolbox = () => {
  const namespace = createNamespace(asyncApi3Namespace);
  const predicates = {
    ...asyncApi3Predicates,
    isStringElement
  };
  return {
    predicates,
    namespace
  };
};
export default createToolbox;