import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import SuccessActionCriteriaElement from "../../../elements/nces/SuccessActionCriteria.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SuccessActionCriteriaVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  element;
  constructor(options) {
    super(options);
    this.element = new SuccessActionCriteriaElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'Criterion'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default SuccessActionCriteriaVisitor;