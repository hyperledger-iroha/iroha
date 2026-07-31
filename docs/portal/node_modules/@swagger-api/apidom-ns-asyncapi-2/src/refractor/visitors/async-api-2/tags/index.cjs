"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _Tags = _interopRequireDefault(require("../../../../elements/Tags.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class TagsVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Tags.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const tagElement = this.toRefractedElement(['document', 'objects', 'Tag'], item);
      this.element.push(tagElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = TagsVisitor;