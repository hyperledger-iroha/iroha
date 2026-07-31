import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { isObjectElement, toValue } from '@swagger-api/apidom-core';
import ResponseElement from "../../../../elements/Response.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isHeaderElement, isMediaTypeElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ResponseVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ResponseElement();
    this.specPath = always(['document', 'objects', 'Response']);
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every MediaTypeElement with media type metadata
    if (isObjectElement(this.element.contentProp)) {
      this.element.contentProp.filter(isMediaTypeElement)
      // @ts-ignore
      .forEach((mediaTypeElement, key) => {
        mediaTypeElement.setMetaProperty('media-type', toValue(key));
      });
    }

    // decorate every MediaTypeElement with media type metadata
    if (isObjectElement(this.element.headers)) {
      this.element.headers.filter(isHeaderElement)
      // @ts-ignore
      .forEach((headerElement, key) => {
        headerElement.setMetaProperty('header-name', toValue(key));
      });
    }
    return result;
  }
}
export default ResponseVisitor;