"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaAllOfVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/**
 * @public
 */
const JSONSchemaAllOfVisitor = exports.JSONSchemaAllOfVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields.allOf;

/**
 * @public
 */
class AllOfVisitor extends JSONSchemaAllOfVisitor {
  ArrayElement(arrayElement) {
    const result = JSONSchemaAllOfVisitor.prototype.ArrayElement.call(this, arrayElement);
    this.element.filter(_apidomNsJsonSchemaDraft.isJSONReferenceElement)
    // @ts-ignore
    .forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
var _default = exports.default = AllOfVisitor;