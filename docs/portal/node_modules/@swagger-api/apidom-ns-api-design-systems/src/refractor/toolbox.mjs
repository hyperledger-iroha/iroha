import { createNamespace, isStringElement } from '@swagger-api/apidom-core';
import * as apiDesignSystemsPredicates from "../predicates.mjs";
import apiDesignSystemNamespace from "../namespace.mjs";
const createToolbox = () => {
  const namespace = createNamespace(apiDesignSystemNamespace);
  const predicates = {
    ...apiDesignSystemsPredicates,
    isStringElement
  };
  return {
    predicates,
    namespace
  };
};
export default createToolbox;