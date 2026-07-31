import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import OperationTraitSecurityElement from "../../../../elements/nces/OperationTraitSecurity.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SecurityVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationTraitSecurityElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const securityRequirementElement = this.toRefractedElement(['document', 'objects', 'SecurityRequirement'], item);
      this.element.push(securityRequirementElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default SecurityVisitor;