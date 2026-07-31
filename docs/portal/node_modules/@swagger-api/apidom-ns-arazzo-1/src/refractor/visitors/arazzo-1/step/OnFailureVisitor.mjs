import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import StepOnFailureElement from "../../../../elements/nces/StepOnFailure.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OnFailureVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new StepOnFailureElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'FailureAction'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default OnFailureVisitor;