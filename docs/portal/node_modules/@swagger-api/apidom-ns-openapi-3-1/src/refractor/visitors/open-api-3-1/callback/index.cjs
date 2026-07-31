"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseCallbackVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Callback = _interopRequireDefault(require("../../../../elements/Callback.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */
const BaseCallbackVisitor = exports.BaseCallbackVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.Callback.$visitor;
/**
 * @public
 */
class CallbackVisitor extends BaseCallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new _Callback.default();
    this.specPath = element => {
      // @ts-ignore
      return (0, _apidomNsOpenapi.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'PathItem'];
    };
  }
  ObjectElement(objectElement) {
    const result = BaseCallbackVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      // @ts-ignore
      referenceElement.setMetaProperty('referenced-element', 'pathItem');
    });
    return result;
  }
}
var _default = exports.default = CallbackVisitor;