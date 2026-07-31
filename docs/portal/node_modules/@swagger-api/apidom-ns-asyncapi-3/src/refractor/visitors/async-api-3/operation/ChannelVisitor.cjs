"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _Reference = _interopRequireDefault(require("../../../../elements/Reference.cjs"));
var _FixedFieldsVisitor = _interopRequireDefault(require("../../generics/FixedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class ChannelVisitor extends (0, _tsMixer.Mixin)(_FixedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Reference.default();
    this.specPath = (0, _ramda.always)(['document', 'objects', 'Reference']);
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = _FixedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);
    if ((0, _predicates.isReferenceLikeElement)(objectElement)) {
      this.element.classes.push('reference-element');
      this.element.setMetaProperty('referenced-element', 'channel');
    }
    return result;
  }
}
var _default = exports.default = ChannelVisitor;