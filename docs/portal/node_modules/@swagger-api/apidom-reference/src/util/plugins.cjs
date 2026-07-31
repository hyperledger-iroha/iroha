"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.run = exports.filter = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _PluginError = _interopRequireDefault(require("../errors/PluginError.cjs"));
/**
 * Filters the given plugins, returning only the ones return `true` for the given method.
 */
const filter = async (method, parameters, plugins) => {
  const pluginResults = await Promise.all(plugins.map((0, _ramdaAdjunct.invokeArgs)([method], parameters)));
  return plugins.filter((plugin, index) => pluginResults[index]);
};

/**
 * Runs the specified method of the given plugins, in order,
 * until one of them returns a successful result.
 * Each method can return a synchronous value or a Promise.
 * If the promise resolves successfully then the result
 * is immediately returned and no further plugins are called.
 * If the promise rejects then the next plugin is called.
 * If ALL plugins fail, then the last error is thrown.
 */
exports.filter = filter;
const run = async (method, parameters, plugins) => {
  let lastError;
  for (const plugin of plugins) {
    try {
      const result = await plugin[method].call(plugin, ...parameters); // eslint-disable-line no-await-in-loop
      return {
        plugin,
        result
      };
    } catch (error) {
      lastError = new _PluginError.default('Error while running plugin', {
        cause: error,
        plugin
      });
    }
  }
  return Promise.reject(lastError);
};
exports.run = run;