import { Mixin } from 'ts-mixer';
import { BREAK, toValue } from '@swagger-api/apidom-core';
import FallbackVisitor from "../FallbackVisitor.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import DefaultContentTypeElement from "../../../elements/DefaultContentType.mjs";
/**
 * @public
 */
/**
 * @public
 */
class DefaultContentTypeVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  StringElement(stringElement) {
    const defaultContentTypeElement = new DefaultContentTypeElement(toValue(stringElement));
    this.copyMetaAndAttributes(stringElement, defaultContentTypeElement);
    this.element = defaultContentTypeElement;
    return BREAK;
  }
}
export default DefaultContentTypeVisitor;