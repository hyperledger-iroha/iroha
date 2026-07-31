import { createNamespace, isStringElement } from '@swagger-api/apidom-core';
import * as jsonSchemaDraft6Predicates from "../predicates.mjs";
import jsonSchemaDraft6Namespace from "../namespace.mjs";
const createToolbox = () => {
  const namespace = createNamespace(jsonSchemaDraft6Namespace);
  const predicates = {
    ...jsonSchemaDraft6Predicates,
    isStringElement
  };
  return {
    predicates,
    namespace
  };
};
export default createToolbox;