"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _Webhooks = _interopRequireDefault(require("../../../elements/nces/Webhooks.cjs"));
var _predicates = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class WebhooksVisitor extends (0, _tsMixer.Mixin)(_apidomNsOpenapi.MapVisitor, _apidomNsOpenapi.FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new _Webhooks.default();
    this.specPath = element => (0, _apidomNsOpenapi.isReferenceLikeElement)(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'PathItem'];
  }
  ObjectElement(objectElement) {
    const result = _apidomNsOpenapi.MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(_predicates.isReferenceElement).forEach(referenceElement => {
      // @ts-ignore
      referenceElement.setMetaProperty('referenced-element', 'pathItem');
    });

    // decorate every PathItemElement with Webhook name metadata
    this.element.filter(_predicates.isPathItemElement)
    // @ts-ignore
    .forEach((pathItemElement, key) => {
      // @ts-ignore
      pathItemElement.setMetaProperty('webhook-name', (0, _apidomCore.toValue)(key));
    });
    return result;
  }
}
var _default = exports.default = WebhooksVisitor;