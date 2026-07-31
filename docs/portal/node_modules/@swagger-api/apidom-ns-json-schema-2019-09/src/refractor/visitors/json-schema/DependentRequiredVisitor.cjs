"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
/**
 * @public
 */
class DependentRequiredVisitor extends _apidomNsJsonSchemaDraft.FallbackVisitor {
  ObjectElement(objectElement) {
    const result = super.enter(objectElement);
    this.element.classes.push('json-schema-dependentRequired');
    return result;
  }
}
var _default = exports.default = DependentRequiredVisitor;