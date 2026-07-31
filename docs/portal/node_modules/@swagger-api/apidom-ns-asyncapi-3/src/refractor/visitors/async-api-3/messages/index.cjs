"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _MapVisitor = _interopRequireDefault(require("../../generics/MapVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _Messages = _interopRequireDefault(require("../../../../elements/Messages.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class MessagesVisitor extends (0, _tsMixer.Mixin)(_MapVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _Messages.default();
    this.element.classes.push('messages');
    this.specPath = element => {
      return (0, _apidomNsAsyncapi.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Message'];
    };
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = _MapVisitor.default.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'message');
    });
    return result;
  }
}
var _default = exports.default = MessagesVisitor;