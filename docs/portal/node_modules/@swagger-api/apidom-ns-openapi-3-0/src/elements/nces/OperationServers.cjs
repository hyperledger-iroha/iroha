"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationServers extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-servers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationServers.primaryClass);
    this.classes.push('servers');
  }
}
var _default = exports.default = OperationServers;