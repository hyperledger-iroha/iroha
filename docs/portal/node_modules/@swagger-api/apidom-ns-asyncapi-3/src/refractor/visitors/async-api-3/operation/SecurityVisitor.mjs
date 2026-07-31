import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import OperationSecurityElement from "../../../../elements/nces/OperationSecurity.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SecurityVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationSecurityElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      let element;
      if (isReferenceLikeElement(item)) {
        element = this.toRefractedElement(['document', 'objects', 'Reference'], item);
        element.setMetaProperty('referenced-element', 'securityScheme');
      } else {
        element = this.toRefractedElement(['document', 'objects', 'SecurityScheme'], item);
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default SecurityVisitor;