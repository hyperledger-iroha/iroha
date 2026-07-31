import { createNamespace, isStringElement } from '@swagger-api/apidom-core';
import * as jsonSchema202012Predicates from "../predicates.mjs";
import jsonSchema202012Namespace from "../namespace.mjs";
const createToolbox = () => {
  const namespace = createNamespace(jsonSchema202012Namespace);
  const predicates = {
    ...jsonSchema202012Predicates,
    isStringElement
  };
  return {
    predicates,
    namespace
  };
};
export default createToolbox;