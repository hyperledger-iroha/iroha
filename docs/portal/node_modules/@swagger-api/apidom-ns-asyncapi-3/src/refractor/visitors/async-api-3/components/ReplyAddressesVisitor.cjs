"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _ComponentsReplyAddresses = _interopRequireDefault(require("../../../../elements/nces/ComponentsReplyAddresses.cjs"));
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ReplyAddressesVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ComponentsReplyAddresses.default();
    this.specPath = element => (0, _apidomNsAsyncapi.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'OperationReplyAddress'];
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_apidomNsAsyncapi.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'operation-reply-address');
    });
    return result;
  }
}
var _default = exports.default = ReplyAddressesVisitor;