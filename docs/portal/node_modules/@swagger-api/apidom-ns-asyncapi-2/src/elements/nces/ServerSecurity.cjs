"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ServerSecurity extends _apidomCore.ArrayElement {
  static primaryClass = 'server-security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ServerSecurity.primaryClass);
  }
}
var _default = exports.default = ServerSecurity;