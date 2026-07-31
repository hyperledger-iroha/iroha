import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
import OperationParametersElement from "../../../../elements/nces/OperationParameters.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ParametersVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationParametersElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = isReferenceLikeElement(item) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Parameter'];
      const element = this.toRefractedElement(specPath, item);
      if (isReferenceElement(element)) {
        element.setMetaProperty('referenced-element', 'parameter');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default ParametersVisitor;