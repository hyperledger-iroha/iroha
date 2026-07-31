"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Servers extends _apidomCore.ArrayElement {
  static primaryClass = 'servers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Servers.primaryClass);
  }
}
var _default = exports.default = Servers;