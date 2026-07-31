"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaPatternPropertiesVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */
const JSONSchemaPatternPropertiesVisitor = exports.JSONSchemaPatternPropertiesVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields.patternProperties;

/**
 * @public
 */
class PatternPropertiesVisitor extends JSONSchemaPatternPropertiesVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaPatternPropertiesVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
var _default = exports.default = PatternPropertiesVisitor;