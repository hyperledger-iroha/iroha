"use strict";

exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
/**
 * @public
 */

/**
 * @public
 */
class ItemsVisitor extends (0, _tsMixer.Mixin)(_apidomNsJsonSchemaDraft.SpecificationVisitor, _apidomNsJsonSchemaDraft.ParentSchemaAwareVisitor, _apidomNsJsonSchemaDraft.FallbackVisitor) {
  ObjectElement(objectElement) {
    this.element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], objectElement);
    return _apidomCore.BREAK;
  }
  ArrayElement(arrayElement) {
    this.element = new _apidomCore.ArrayElement();
    this.element.classes.push('json-schema-items');
    arrayElement.forEach(item => {
      const element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
  BooleanElement(booleanElement) {
    this.element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], booleanElement);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ItemsVisitor;