"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
/**
 * @public
 */
class PluginError extends _apidomError.ApiDOMError {
  plugin;
  constructor(message, options) {
    super(message, {
      cause: options.cause
    });
    this.plugin = options.plugin;
  }
}
var _default = exports.default = PluginError;