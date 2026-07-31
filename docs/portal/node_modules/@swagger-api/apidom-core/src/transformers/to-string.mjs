import defaultNamespaceInstance from "../namespace.mjs";
import dehydrate from "./dehydrate.mjs";
/**
 * Create a refracted string representation of an Element.
 * @public
 */
const toString = (element, namespace = defaultNamespaceInstance) => {
  const refractStructure = dehydrate(element, namespace);
  return JSON.stringify(refractStructure);
};
export default toString;