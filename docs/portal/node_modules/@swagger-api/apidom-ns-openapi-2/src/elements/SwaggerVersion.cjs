"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SwaggerVersion extends _apidomCore.StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'swaggerVersion';
    this.classes.push('spec-version');
    this.classes.push('version');
  }
}
var _default = exports.default = SwaggerVersion;