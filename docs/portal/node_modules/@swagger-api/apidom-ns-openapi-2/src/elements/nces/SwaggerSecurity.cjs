"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SwaggerSecurity extends _apidomCore.ArrayElement {
  static primaryClass = 'swagger-security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerSecurity.primaryClass);
    this.classes.push('security');
  }
}
var _default = exports.default = SwaggerSecurity;