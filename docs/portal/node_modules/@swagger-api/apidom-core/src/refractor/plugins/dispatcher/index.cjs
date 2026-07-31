"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.dispatchPluginsSync = exports.dispatchPluginsAsync = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _toolbox = _interopRequireDefault(require("../../toolbox.cjs"));
var _visitor = require("../../../traversal/visitor.cjs");
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
  toolboxCreator: _toolbox.default,
  visitorOptions: {
    nodeTypeGetter: _visitor.getNodeType,
    exposeEdits: true
  }
};

/**
 * @public
 */
const dispatchPluginsSync = (element, plugins, options = {}) => {
  if (plugins.length === 0) return element;
  const mergedOptions = (0, _ramda.mergeDeepRight)(defaultDispatchPluginsOptions, options);
  const {
    toolboxCreator,
    visitorOptions
  } = mergedOptions;
  const toolbox = toolboxCreator();
  const pluginsSpecs = plugins.map(plugin => plugin(toolbox));
  const mergedPluginsVisitor = (0, _visitor.mergeAllVisitors)(pluginsSpecs.map((0, _ramda.propOr)({}, 'visitor')), {
    ...visitorOptions
  });
  pluginsSpecs.forEach((0, _ramdaAdjunct.invokeArgs)(['pre'], []));
  const newElement = (0, _visitor.visit)(element, mergedPluginsVisitor, visitorOptions);
  pluginsSpecs.forEach((0, _ramdaAdjunct.invokeArgs)(['post'], []));
  return newElement;
};
exports.dispatchPluginsSync = dispatchPluginsSync;
const dispatchPluginsAsync = async (element, plugins, options = {}) => {
  if (plugins.length === 0) return element;
  const mergedOptions = (0, _ramda.mergeDeepRight)(defaultDispatchPluginsOptions, options);
  const {
    toolboxCreator,
    visitorOptions
  } = mergedOptions;
  const toolbox = toolboxCreator();
  const pluginsSpecs = plugins.map(plugin => plugin(toolbox));
  const mergeAllVisitorsAsync = _visitor.mergeAllVisitors[Symbol.for('nodejs.util.promisify.custom')];
  const visitAsync = _visitor.visit[Symbol.for('nodejs.util.promisify.custom')];
  const mergedPluginsVisitor = mergeAllVisitorsAsync(pluginsSpecs.map((0, _ramda.propOr)({}, 'visitor')), {
    ...visitorOptions
  });
  await Promise.allSettled(pluginsSpecs.map((0, _ramdaAdjunct.invokeArgs)(['pre'], [])));
  const newElement = await visitAsync(element, mergedPluginsVisitor, visitorOptions);
  await Promise.allSettled(pluginsSpecs.map((0, _ramdaAdjunct.invokeArgs)(['post'], [])));
  return newElement;
};
exports.dispatchPluginsAsync = dispatchPluginsAsync;
dispatchPluginsSync[Symbol.for('nodejs.util.promisify.custom')] = dispatchPluginsAsync;