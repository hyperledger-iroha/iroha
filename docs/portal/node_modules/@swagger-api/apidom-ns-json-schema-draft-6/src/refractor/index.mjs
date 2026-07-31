import { path } from 'ramda';
import { visit, dereference, refract as baseRefract, dispatchRefractorPlugins } from '@swagger-api/apidom-core';
import specification from "./specification.mjs";
import { keyMap, getNodeType } from "../traversal/visitor.mjs";
import createToolbox from "./toolbox.mjs";
/**
 * @public
 */
const refract = (value, {
  specPath = ['visitors', 'document', 'objects', 'JSONSchema', '$visitor'],
  plugins = [],
  specificationObj = specification
} = {}) => {
  const element = baseRefract(value);
  const resolvedSpec = dereference(specificationObj);

  /**
   * This is where generic ApiDOM becomes semantic (namespace applied).
   * We don't allow consumers to hook into this translation.
   * Though we allow consumers to define their onw plugins on already transformed ApiDOM.
   */
  const RootVisitorClass = path(specPath, resolvedSpec);
  const rootVisitor = new RootVisitorClass({
    specObj: resolvedSpec
  });
  visit(element, rootVisitor);

  /**
   * Running plugins visitors means extra single traversal === performance hit.
   */
  return dispatchRefractorPlugins(rootVisitor.element, plugins, {
    toolboxCreator: createToolbox,
    visitorOptions: {
      keyMap,
      nodeTypeGetter: getNodeType
    }
  });
};

/**
 * @public
 */
export const createRefractor = specPath => (value, options = {}) => refract(value, {
  specPath,
  ...options
});
export default refract;