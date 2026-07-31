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
class AnyOfVisitor extends (0, _tsMixer.Mixin)(_apidomNsJsonSchemaDraft.SpecificationVisitor, _apidomNsJsonSchemaDraft.ParentSchemaAwareVisitor, _apidomNsJsonSchemaDraft.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _apidomCore.ArrayElement();
    this.element.classes.push('json-schema-anyOf');
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = AnyOfVisitor;