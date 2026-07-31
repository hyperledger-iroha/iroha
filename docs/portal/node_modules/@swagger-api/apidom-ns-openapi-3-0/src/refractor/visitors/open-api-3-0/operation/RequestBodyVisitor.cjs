"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _predicates = require("../../../../predicates.cjs");
var _predicates2 = require("../../../predicates.cjs");
var _AlternatingVisitor = _interopRequireDefault(require("../../generics/AlternatingVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class RequestBodyVisitor extends _AlternatingVisitor.default {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: _predicates2.isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: _ramda.T,
      specPath: ['document', 'objects', 'RequestBody']
    }];
  }
  ObjectElement(objectElement) {
    const result = _AlternatingVisitor.default.prototype.enter.call(this, objectElement);
    if ((0, _predicates.isReferenceElement)(this.element)) {
      this.element.setMetaProperty('referenced-element', 'requestBody');
    }
    return result;
  }
}
var _default = exports.default = RequestBodyVisitor;