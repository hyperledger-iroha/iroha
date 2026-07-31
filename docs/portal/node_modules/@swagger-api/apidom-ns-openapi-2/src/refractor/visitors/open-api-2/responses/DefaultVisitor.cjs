"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _ramda = require("ramda");
var _predicates = require("../../../predicates.cjs");
var _predicates2 = require("../../../../predicates.cjs");
var _AlternatingVisitor = _interopRequireDefault(require("../../generics/AlternatingVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class DefaultVisitor extends (0, _tsMixer.Mixin)(_AlternatingVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: _predicates.isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: _ramda.T,
      specPath: ['document', 'objects', 'Response']
    }];
  }
  ObjectElement(objectElement) {
    const result = _AlternatingVisitor.default.prototype.enter.call(this, objectElement);

    // decorate ReferenceElement with type of referencing element
    if ((0, _predicates2.isReferenceElement)(this.element)) {
      this.element.setMetaProperty('referenced-element', 'response');
    } else if ((0, _predicates2.isResponseElement)(this.element)) {
      this.element.setMetaProperty('http-status-code', 'default');
    }
    return result;
  }
}
var _default = exports.default = DefaultVisitor;