import { invokeArgs } from 'ramda-adjunct';
import PluginError from "../errors/PluginError.mjs";
/**
 * Filters the given plugins, returning only the ones return `true` for the given method.
 */
export const filter = async (method, parameters, plugins) => {
  const pluginResults = await Promise.all(plugins.map(invokeArgs([method], parameters)));
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
export const run = async (method, parameters, plugins) => {
  let lastError;
  for (const plugin of plugins) {
    try {
      const result = await plugin[method].call(plugin, ...parameters); // eslint-disable-line no-await-in-loop
      return {
        plugin,
        result
      };
    } catch (error) {
      lastError = new PluginError('Error while running plugin', {
        cause: error,
        plugin
      });
    }
  }
  return Promise.reject(lastError);
};