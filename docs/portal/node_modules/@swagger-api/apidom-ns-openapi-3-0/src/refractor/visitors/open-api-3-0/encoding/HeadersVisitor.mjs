import { Mixin } from 'ts-mixer';
import { toValue } from '@swagger-api/apidom-core';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isHeaderElement, isReferenceElement } from "../../../../predicates.mjs";
import EncodingHeadersElement from "../../../../elements/nces/EncodingHeaders.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HeadersVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new EncodingHeadersElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Header'];
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'header');
    });

    // decorate every HeaderElement with metadata about their name
    this.element.forEach((value, key) => {
      if (!isHeaderElement(value)) return;
      const headerName = toValue(key);
      value.setMetaProperty('headerName', headerName);
    });
    return result;
  }
}
export default HeadersVisitor;