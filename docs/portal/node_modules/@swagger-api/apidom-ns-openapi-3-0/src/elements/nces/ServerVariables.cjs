"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ServerVariables extends _apidomCore.ObjectElement {
  static primaryClass = 'server-variables';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ServerVariables.primaryClass);
  }
}
var _default = exports.default = ServerVariables;