"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ServerVariables = _interopRequireDefault(require("../../../../elements/nces/ServerVariables.cjs"));
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../predicates.cjs");
var _predicates2 = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class VariablesVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ServerVariables.default();
    this.specPath = element => {
      return (0, _predicates.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'ServerVariable'];
    };
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-expect-error
    this.element.filter(_predicates2.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'serverVariable');
    });
    return result;
  }
}
var _default = exports.default = VariablesVisitor;