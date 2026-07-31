"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaTypeVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/**
 * @public
 */
const JSONSchemaTypeVisitor = exports.JSONSchemaTypeVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields.type;

/**
 * @public
 */
class TypeVisitor extends JSONSchemaTypeVisitor {
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    return result;
  }
}
var _default = exports.default = TypeVisitor;