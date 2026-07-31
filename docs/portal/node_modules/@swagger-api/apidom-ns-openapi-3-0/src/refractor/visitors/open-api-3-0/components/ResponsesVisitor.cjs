"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _ComponentsResponses = _interopRequireDefault(require("../../../../elements/nces/ComponentsResponses.cjs"));
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
class ResponsesVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ComponentsResponses.default();
    this.specPath = element => (0, _predicates.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Response'];
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(_predicates2.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'response');
    });

    // decorate every ResponseElement with metadata about their status code
    // @ts-ignore
    this.element.filter(_predicates2.isResponseElement).forEach((value, key) => {
      value.setMetaProperty('http-status-code', (0, _apidomCore.toValue)(key));
    });
    return result;
  }
}
var _default = exports.default = ResponsesVisitor;