"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SwaggerTags extends _apidomCore.ArrayElement {
  static primaryClass = 'swagger-tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerTags.primaryClass);
  }
}
var _default = exports.default = SwaggerTags;