import { dispatchPluginsSync } from "./plugins/dispatcher/index.mjs";
import { getNodeType } from "../traversal/visitor.mjs";
import { cloneDeep } from "../clone/index.mjs";
import { isElement } from "../predicates/index.mjs";
import createToolbox from "./toolbox.mjs";
const refract = (value, {
  Type,
  plugins = []
}) => {
  /**
   * This is where values gets refracted into generic ApiDOM.
   * We don't allow consumers to hook into this translation.
   * Though we allow consumers to define their onw plugins on already transformed ApiDOM.
   */
  const element = new Type(value);
  if (isElement(value)) {
    if (value.meta.length > 0) {
      element.meta = cloneDeep(value.meta);
    }
    if (value.attributes.length > 0) {
      element.attributes = cloneDeep(value.attributes);
    }
  }

  /**
   * Run plugins only when necessary.
   * Running plugins visitors means extra single traversal === performance hit.
   */
  return dispatchPluginsSync(element, plugins, {
    toolboxCreator: createToolbox,
    visitorOptions: {
      nodeTypeGetter: getNodeType
    }
  });
};
export const createRefractor = Type => (value, options = {}) => refract(value, {
  ...options,
  Type
});
export default refract;