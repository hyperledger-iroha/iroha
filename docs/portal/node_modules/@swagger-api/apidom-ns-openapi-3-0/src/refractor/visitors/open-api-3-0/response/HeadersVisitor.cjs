"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _ResponseHeaders = _interopRequireDefault(require("../../../../elements/nces/ResponseHeaders.cjs"));
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
class HeadersVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ResponseHeaders.default();
    this.specPath = element => (0, _predicates.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Header'];
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(_predicates2.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'header');
    });

    // decorate every HeaderElement with metadata about their name
    this.element.forEach((value, key) => {
      if (!(0, _predicates2.isHeaderElement)(value)) return;
      const headerName = (0, _apidomCore.toValue)(key);
      value.setMetaProperty('header-name', headerName);
    });
    return result;
  }
}
var _default = exports.default = HeadersVisitor;