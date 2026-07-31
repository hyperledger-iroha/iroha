import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import SourceDescriptionsElement from "../../../elements/nces/SourceDescriptions.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SourceDescriptionsVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  element;
  constructor(options) {
    super(options);
    this.element = new SourceDescriptionsElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'SourceDescription'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default SourceDescriptionsVisitor;