"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2019-09");
var _JSONSchema = _interopRequireDefault(require("../../../elements/JSONSchema.cjs"));
/**
 * @public
 */
class JSONSchemaVisitor extends _apidomNsJsonSchema.JSONSchemaVisitor {
  constructor(options) {
    super(options);
    this.element = new _JSONSchema.default();
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'https://json-schema.org/draft/2020-12/schema';
  }
}
var _default = exports.default = JSONSchemaVisitor;