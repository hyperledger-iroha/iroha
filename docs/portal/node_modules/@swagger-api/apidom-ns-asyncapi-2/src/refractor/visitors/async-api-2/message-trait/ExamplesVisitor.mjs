import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import MessageTraitExamplesElement from "../../../../elements/nces/MessageTraitExamples.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ExamplesVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessageTraitExamplesElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const messageExampleElement = this.toRefractedElement(['document', 'objects', 'MessageExample'], item);
      this.element.push(messageExampleElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default ExamplesVisitor;