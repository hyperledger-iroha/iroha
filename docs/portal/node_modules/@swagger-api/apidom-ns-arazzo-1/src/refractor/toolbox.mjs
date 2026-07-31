import { createNamespace, isStringElement } from '@swagger-api/apidom-core';
import * as workflowsPredicates from "../predicates.mjs";
import * as refractorPredicates from "./predicates.mjs";
import workflowsNamespace from "../namespace.mjs";
const createToolbox = () => {
  const namespace = createNamespace(workflowsNamespace);
  const predicates = {
    ...refractorPredicates,
    ...workflowsPredicates,
    isStringElement
  };
  return {
    predicates,
    namespace
  };
};
export default createToolbox;