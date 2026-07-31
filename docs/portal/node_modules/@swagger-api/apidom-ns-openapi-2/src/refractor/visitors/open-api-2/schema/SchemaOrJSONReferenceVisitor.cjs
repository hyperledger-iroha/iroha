"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaOrJSONReferenceVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/**
 * @public
 */
// eslint-disable-next-line prefer-destructuring
const JSONSchemaOrJSONReferenceVisitor = exports.JSONSchemaOrJSONReferenceVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.JSONSchemaOrJSONReferenceVisitor;

/**
 * @public
 */
class SchemaOrJSONReferenceVisitor extends JSONSchemaOrJSONReferenceVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaOrJSONReferenceVisitor.prototype.enter.call(this, objectElement);
    if ((0, _apidomNsJsonSchemaDraft.isJSONReferenceElement)(this.element)) {
      this.element.setMetaProperty('referenced-element', 'schema');
    }
    return result;
  }
}
var _default = exports.default = SchemaOrJSONReferenceVisitor;