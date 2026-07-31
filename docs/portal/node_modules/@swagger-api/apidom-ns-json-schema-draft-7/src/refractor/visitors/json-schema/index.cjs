"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-6");
var _JSONSchema = _interopRequireDefault(require("../../../elements/JSONSchema.cjs"));
/**
 * @public
 */
class JSONSchemaVisitor extends _apidomNsJsonSchemaDraft.JSONSchemaVisitor {
  constructor(options) {
    super(options);
    this.element = new _JSONSchema.default();
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'http://json-schema.org/draft-07/schema#';
  }
}
var _default = exports.default = JSONSchemaVisitor;