"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaDependenciesVisitor = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */
const JSONSchemaDependenciesVisitor = exports.JSONSchemaDependenciesVisitor = _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields.dependencies;

/**
 * @public
 */
class DependenciesVisitor extends JSONSchemaDependenciesVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaDependenciesVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
var _default = exports.default = DependenciesVisitor;