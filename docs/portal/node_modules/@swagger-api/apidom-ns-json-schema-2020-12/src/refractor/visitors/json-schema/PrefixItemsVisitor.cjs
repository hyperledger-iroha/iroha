"use strict";

exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2019-09");
/**
 * @public
 */

/**
 * @public
 */
class PrefixItemsVisitor extends (0, _tsMixer.Mixin)(_apidomNsJsonSchema.SpecificationVisitor, _apidomNsJsonSchema.ParentSchemaAwareVisitor, _apidomNsJsonSchema.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _apidomCore.ArrayElement();
    this.element.classes.push('json-schema-prefixItems');
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
var _default = exports.default = PrefixItemsVisitor;