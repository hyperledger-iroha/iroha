"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _ParentSchemaAwareVisitor = _interopRequireDefault(require("./ParentSchemaAwareVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class LinksVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _ParentSchemaAwareVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _apidomCore.ArrayElement();
    this.element.classes.push('json-schema-links');
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const linkDescriptionElement = this.toRefractedElement(['document', 'objects', 'LinkDescription'], item);
      this.element.push(linkDescriptionElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = LinksVisitor;