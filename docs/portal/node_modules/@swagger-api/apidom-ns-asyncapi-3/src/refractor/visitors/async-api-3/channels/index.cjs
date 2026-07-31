"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _Channels = _interopRequireDefault(require("../../../../elements/Channels.cjs"));
var _predicates = require("../../../../predicates.cjs");
var _predicates2 = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class ChannelsVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Channels.default();
    this.specPath = element => (0, _predicates2.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Channel'];
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'channel');
    });
    return result;
  }
}
var _default = exports.default = ChannelsVisitor;