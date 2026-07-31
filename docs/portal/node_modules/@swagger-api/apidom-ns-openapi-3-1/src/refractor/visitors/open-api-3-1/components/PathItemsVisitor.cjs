"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _ComponentsPathItems = _interopRequireDefault(require("../../../../elements/nces/ComponentsPathItems.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class PathItemsVisitor extends (0, _tsMixer.Mixin)(_apidomNsOpenapi.MapVisitor, _apidomNsOpenapi.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _ComponentsPathItems.default();
    this.specPath = element => (0, _apidomNsOpenapi.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'PathItem'];
  }
  ObjectElement(objectElement) {
    const result = _apidomNsOpenapi.MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      // @ts-ignore
      referenceElement.setMetaProperty('referenced-element', 'pathItem');
    });
    return result;
  }
}
var _default = exports.default = PathItemsVisitor;