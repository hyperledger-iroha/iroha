import { mergeDeepRight, propOr } from 'ramda';
import { invokeArgs } from 'ramda-adjunct';
import createToolbox from "../../toolbox.mjs";
import { getNodeType, mergeAllVisitors, visit } from "../../../traversal/visitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
const defaultDispatchPluginsOptions = {
  toolboxCreator: createToolbox,
  visitorOptions: {
    nodeTypeGetter: getNodeType,
    exposeEdits: true
  }
};

/**
 * @public
 */
export const dispatchPluginsSync = (element, plugins, options = {}) => {
  if (plugins.length === 0) return element;
  const mergedOptions = mergeDeepRight(defaultDispatchPluginsOptions, options);
  const {
    toolboxCreator,
    visitorOptions
  } = mergedOptions;
  const toolbox = toolboxCreator();
  const pluginsSpecs = plugins.map(plugin => plugin(toolbox));
  const mergedPluginsVisitor = mergeAllVisitors(pluginsSpecs.map(propOr({}, 'visitor')), {
    ...visitorOptions
  });
  pluginsSpecs.forEach(invokeArgs(['pre'], []));
  const newElement = visit(element, mergedPluginsVisitor, visitorOptions);
  pluginsSpecs.forEach(invokeArgs(['post'], []));
  return newElement;
};
export const dispatchPluginsAsync = async (element, plugins, options = {}) => {
  if (plugins.length === 0) return element;
  const mergedOptions = mergeDeepRight(defaultDispatchPluginsOptions, options);
  const {
    toolboxCreator,
    visitorOptions
  } = mergedOptions;
  const toolbox = toolboxCreator();
  const pluginsSpecs = plugins.map(plugin => plugin(toolbox));
  const mergeAllVisitorsAsync = mergeAllVisitors[Symbol.for('nodejs.util.promisify.custom')];
  const visitAsync = visit[Symbol.for('nodejs.util.promisify.custom')];
  const mergedPluginsVisitor = mergeAllVisitorsAsync(pluginsSpecs.map(propOr({}, 'visitor')), {
    ...visitorOptions
  });
  await Promise.allSettled(pluginsSpecs.map(invokeArgs(['pre'], [])));
  const newElement = await visitAsync(element, mergedPluginsVisitor, visitorOptions);
  await Promise.allSettled(pluginsSpecs.map(invokeArgs(['post'], [])));
  return newElement;
};
dispatchPluginsSync[Symbol.for('nodejs.util.promisify.custom')] = dispatchPluginsAsync;