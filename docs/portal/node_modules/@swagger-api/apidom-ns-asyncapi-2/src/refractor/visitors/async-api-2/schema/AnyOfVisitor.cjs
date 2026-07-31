"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaAnyOfVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */
const JSONSchemaAnyOfVisitor = exports.JSONSchemaAnyOfVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields.anyOf;

/**
 * @public
 */
class AnyOfVisitor extends JSONSchemaAnyOfVisitor {
  ArrayElement(arrayElement) {
    const result = JSONSchemaAnyOfVisitor.prototype.ArrayElement.call(this, arrayElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
var _default = exports.default = AnyOfVisitor;