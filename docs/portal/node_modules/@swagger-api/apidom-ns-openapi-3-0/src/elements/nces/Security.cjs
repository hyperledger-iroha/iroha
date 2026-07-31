"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Security extends _apidomCore.ArrayElement {
  static primaryClass = 'security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Security.primaryClass);
  }
}
var _default = exports.default = Security;