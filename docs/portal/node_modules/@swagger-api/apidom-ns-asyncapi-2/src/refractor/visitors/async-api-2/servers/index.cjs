"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _PatternedFieldsVisitor = _interopRequireDefault(require("../../generics/PatternedFieldsVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _Servers = _interopRequireDefault(require("../../../../elements/Servers.cjs"));
var _predicates = require("../../../predicates.cjs");
var _predicates2 = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class ServersVisitor extends (0, _tsMixer.Mixin)(_PatternedFieldsVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Servers.default();
    this.element.classes.push('servers');
    this.specPath = element => {
      return (0, _predicates.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Server'];
    };
    this.canSupportSpecificationExtensions = false;
    // @ts-ignore
    this.fieldPatternPredicate = (0, _ramda.test)(/^[A-Za-z0-9_-]+$/);
  }
  ObjectElement(objectElement) {
    const result = _PatternedFieldsVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates2.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'server');
    });
    return result;
  }
}
var _default = exports.default = ServersVisitor;