"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _AlternatingVisitor = _interopRequireDefault(require("../../generics/AlternatingVisitor.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */
class ExternalDocumentationOrReferenceVisitor extends _AlternatingVisitor.default {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: _apidomNsAsyncapi.isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: _ramda.T,
      specPath: ['document', 'objects', 'ExternalDocumentation']
    }];
  }
  enter(element) {
    const result = _AlternatingVisitor.default.prototype.enter.call(this, element);
    if ((0, _predicates.isReferenceElement)(this.element)) {
      this.element.setMetaProperty('referenced-element', 'externalDocumentation');
    }
    return result;
  }
}
var _default = exports.default = ExternalDocumentationOrReferenceVisitor;