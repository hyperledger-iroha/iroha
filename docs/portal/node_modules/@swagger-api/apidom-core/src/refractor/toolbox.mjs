import * as basePredicates from "../predicates/index.mjs";
import defaultNamespaceInstance from "../namespace.mjs";
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
  const predicates = {
    ...basePredicates
  };
  return {
    predicates,
    namespace: defaultNamespaceInstance
  };
};
export default createToolbox;