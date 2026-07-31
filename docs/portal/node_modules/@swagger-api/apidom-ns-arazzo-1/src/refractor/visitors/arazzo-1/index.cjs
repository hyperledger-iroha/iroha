"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _FixedFieldsVisitor = _interopRequireDefault(require("../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _ArazzoSpecification = _interopRequireDefault(require("../../../elements/ArazzoSpecification1.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ArazzoSpecificationVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  element;
  specPath;
  canSupportSpecificationExtensions;
  constructor(options) {
    super(options);
    this.element = new _ArazzoSpecification.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'ArazzoSpecification']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    return _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
  }
}
var _default = exports.default = ArazzoSpecificationVisitor;