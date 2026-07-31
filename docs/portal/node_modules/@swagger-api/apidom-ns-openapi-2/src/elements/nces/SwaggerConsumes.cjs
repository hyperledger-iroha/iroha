"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SwaggerConsumes extends _apidomCore.ArrayElement {
  static primaryClass = 'swagger-consumes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerConsumes.primaryClass);
  }
}
var _default = exports.default = SwaggerConsumes;