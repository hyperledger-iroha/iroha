"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SwaggerSchemes extends _apidomCore.ArrayElement {
  static primaryClass = 'swagger-schemes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerSchemes.primaryClass);
  }
}
var _default = exports.default = SwaggerSchemes;