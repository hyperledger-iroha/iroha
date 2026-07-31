import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import FailureActionCriteriaElement from "../../../elements/nces/FailureActionCriteria.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class FailureActionCriteriaVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  element;
  constructor(options) {
    super(options);
    this.element = new FailureActionCriteriaElement();
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
export default FailureActionCriteriaVisitor;