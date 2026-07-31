import { Mixin } from 'ts-mixer';
import { isObjectElement, BREAK } from '@swagger-api/apidom-core';
import OperationSecurityElement from "../../../../elements/nces/OperationSecurity.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
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
      const specPath = isObjectElement(item) ? ['document', 'objects', 'SecurityRequirement'] : ['value'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default SecurityVisitor;