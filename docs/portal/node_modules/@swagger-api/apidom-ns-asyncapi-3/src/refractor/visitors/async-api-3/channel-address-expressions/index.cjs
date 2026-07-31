"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _PatternedFieldsVisitor = _interopRequireDefault(require("../../generics/PatternedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _Message = _interopRequireDefault(require("../../../../elements/Message.cjs"));
var _predicates = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class ChannelAddressExpressionsVisitor extends (0, _tsMixer.Mixin)(_PatternedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Message.default();
    this.element.classes.push('servers');
    this.specPath = element => {
      return (0, _predicates.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Message'];
    };
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = _PatternedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'message');
    });
    return result;
  }
}
var _default = exports.default = ChannelAddressExpressionsVisitor;