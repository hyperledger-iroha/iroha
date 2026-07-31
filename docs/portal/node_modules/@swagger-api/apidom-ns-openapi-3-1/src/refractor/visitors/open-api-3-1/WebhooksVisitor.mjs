import { Mixin } from 'ts-mixer';
import { toValue } from '@swagger-api/apidom-core';
import { isReferenceLikeElement, MapVisitor, FallbackVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import WebhooksElement from "../../../elements/nces/Webhooks.mjs";
import { isPathItemElement, isReferenceElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class WebhooksVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new WebhooksElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'PathItem'];
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      // @ts-ignore
      referenceElement.setMetaProperty('referenced-element', 'pathItem');
    });

    // decorate every PathItemElement with Webhook name metadata
    this.element.filter(isPathItemElement)
    // @ts-ignore
    .forEach((pathItemElement, key) => {
      // @ts-ignore
      pathItemElement.setMetaProperty('webhook-name', toValue(key));
    });
    return result;
  }
}
export default WebhooksVisitor;