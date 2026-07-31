"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class PathItemServers extends _apidomCore.ArrayElement {
  static primaryClass = 'path-item-servers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(PathItemServers.primaryClass);
    this.classes.push('servers');
  }
}
var _default = exports.default = PathItemServers;