"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaOneOfVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */
const JSONSchemaOneOfVisitor = exports.JSONSchemaOneOfVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields.oneOf;

/**
 * @public
 */
class OneOfVisitor extends JSONSchemaOneOfVisitor {
  ArrayElement(arrayElement) {
    const result = JSONSchemaOneOfVisitor.prototype.ArrayElement.call(this, arrayElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
var _default = exports.default = OneOfVisitor;