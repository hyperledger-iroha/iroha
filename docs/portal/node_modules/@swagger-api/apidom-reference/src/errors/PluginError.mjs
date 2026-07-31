import { ApiDOMError } from '@swagger-api/apidom-error';

/**
 * @public
 */
class PluginError extends ApiDOMError {
  plugin;
  constructor(message, options) {
    super(message, {
      cause: options.cause
    });
    this.plugin = options.plugin;
  }
}
export default PluginError;