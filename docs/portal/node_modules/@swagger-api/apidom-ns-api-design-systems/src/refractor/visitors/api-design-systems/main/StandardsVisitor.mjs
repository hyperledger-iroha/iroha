import { Mixin } from 'ts-mixer';
import { ArrayElement, BREAK } from '@swagger-api/apidom-core';
import FallbackVisitor from "../../FallbackVisitor.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class StandardsVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ArrayElement();
    this.element.classes.push('main-standards');
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'Standard'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default StandardsVisitor;