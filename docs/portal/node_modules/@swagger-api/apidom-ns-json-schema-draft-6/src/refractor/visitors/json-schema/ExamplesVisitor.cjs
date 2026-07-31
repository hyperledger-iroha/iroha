"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/**
 * @public
 */
class ExamplesVisitor extends _apidomNsJsonSchemaDraft.FallbackVisitor {
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    this.element.classes.push('json-schema-examples');
    return result;
  }
}
var _default = exports.default = ExamplesVisitor;