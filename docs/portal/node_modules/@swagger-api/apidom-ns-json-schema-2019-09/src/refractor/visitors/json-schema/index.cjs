"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
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
    return 'https://json-schema.org/draft/2019-09/schema';
  }
  ObjectElement(objectElement) {
    this.handleDialectIdentifier(objectElement);
    this.handleSchemaIdentifier(objectElement);

    // for further processing consider this Schema Element as parent for all embedded Schema Elements
    this.parent = this.element;
    const result = _apidomNsJsonSchemaDraft.FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // mark this SchemaElement with reference metadata
    if ((0, _apidomCore.isStringElement)(this.element.$ref)) {
      this.element.classes.push('reference-element');
      this.element.setMetaProperty('referenced-element', 'schema');
    }
    return result;
  }
}
var _default = exports.default = JSONSchemaVisitor;