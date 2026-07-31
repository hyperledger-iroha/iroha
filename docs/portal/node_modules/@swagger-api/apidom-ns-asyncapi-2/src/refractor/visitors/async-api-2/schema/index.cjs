"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _Schema = _interopRequireDefault(require("../../../../elements/Schema.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class SchemaVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Schema']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    this.element = new _Schema.default();
    return _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
  }
  BooleanElement(booleanElement) {
    const result = super.enter(booleanElement);
    this.element.classes.push('boolean-json-schema');
    return result;
  }
}
var _default = exports.default = SchemaVisitor;