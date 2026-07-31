"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/**
 * @public
 */
class ItemsVisitor extends _apidomNsJsonSchemaDraft.ItemsVisitor {
  BooleanElement(booleanElement) {
    this.element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], booleanElement);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ItemsVisitor;