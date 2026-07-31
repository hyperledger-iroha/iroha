import { Mixin } from 'ts-mixer';
import { toValue } from '@swagger-api/apidom-core';
import ComponentsResponsesElement from "../../../../elements/nces/ComponentsResponses.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement, isResponseElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ResponsesVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsResponsesElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Response'];
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'response');
    });

    // decorate every ResponseElement with metadata about their status code
    // @ts-ignore
    this.element.filter(isResponseElement).forEach((value, key) => {
      value.setMetaProperty('http-status-code', toValue(key));
    });
    return result;
  }
}
export default ResponsesVisitor;