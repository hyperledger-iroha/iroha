"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2020-12");
var _predicates = require("../../../../predicates.cjs");
var _Schema = _interopRequireDefault(require("../../../../elements/Schema.cjs"));
var _JsonSchemaDialect = _interopRequireDefault(require("../../../../elements/JsonSchemaDialect.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class SchemaVisitor extends (0, _tsMixer.Mixin)(_apidomNsOpenapi.FixedFieldsVisitor, _apidomNsJsonSchema.ParentSchemaAwareVisitor, _apidomNsOpenapi.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _Schema.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Schema']);
    this.canSupportSpecificationExtensions = true;
    this.jsonSchemaDefaultDialect = _JsonSchemaDialect.default.default;
    this.passingOptionsNames.push('parent');
  }
  ObjectElement(objectElement) {
    this.handleDialectIdentifier(objectElement);
    this.handleSchemaIdentifier(objectElement);

    // for further processing consider this Schema Element as parent for all embedded Schema Elements
    this.parent = this.element;
    const result = _apidomNsOpenapi.FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // mark this SchemaElement with reference metadata
    if ((0, _apidomCore.isStringElement)(this.element.$ref)) {
      this.element.classes.push('reference-element');
      this.element.setMetaProperty('referenced-element', 'schema');
    }
    return result;
  }
  BooleanElement(booleanElement) {
    return _apidomNsJsonSchema.JSONSchemaVisitor.prototype.BooleanElement.call(this, booleanElement);
  }

  /**
   * This function depends on some external context, so we need to make sure this function
   * works even when no context is provided like when directly refracting generic Object Element
   * into Schema Element: `SchemaElement.refract(new ObjectElement({ type: 'object' });`
   */
  get defaultDialectIdentifier() {
    let jsonSchemaDialect;
    if (this.openApiSemanticElement !== undefined &&
    // @ts-ignore
    (0, _predicates.isJsonSchemaDialectElement)(this.openApiSemanticElement.jsonSchemaDialect)) {
      // @ts-ignore
      jsonSchemaDialect = (0, _apidomCore.toValue)(this.openApiSemanticElement.jsonSchemaDialect);
    } else if (this.openApiGenericElement !== undefined && (0, _apidomCore.isStringElement)(this.openApiGenericElement.get('jsonSchemaDialect'))) {
      jsonSchemaDialect = (0, _apidomCore.toValue)(this.openApiGenericElement.get('jsonSchemaDialect'));
    } else {
      jsonSchemaDialect = (0, _apidomCore.toValue)(this.jsonSchemaDefaultDialect);
    }
    return jsonSchemaDialect;
  }
  handleDialectIdentifier(objectElement) {
    return _apidomNsJsonSchema.JSONSchemaVisitor.prototype.handleDialectIdentifier.call(this, objectElement);
  }
  handleSchemaIdentifier(objectElement) {
    return _apidomNsJsonSchema.JSONSchemaVisitor.prototype.handleSchemaIdentifier.call(this, objectElement);
  }
}
var _default = exports.default = SchemaVisitor;