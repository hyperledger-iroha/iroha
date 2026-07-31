"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class JsonSchemaDialect extends _apidomCore.StringElement {
  static default = new JsonSchemaDialect('https://spec.openapis.org/oas/3.1/dialect/base');
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'jsonSchemaDialect';
  }
}
var _default = exports.default = JsonSchemaDialect;