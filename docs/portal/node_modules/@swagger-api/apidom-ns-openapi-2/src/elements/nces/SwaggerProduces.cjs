"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SwaggerProduces extends _apidomCore.ArrayElement {
  static primaryClass = 'swagger-produces';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerProduces.primaryClass);
  }
}
var _default = exports.default = SwaggerProduces;