import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import WorkflowsElement from "../../../elements/nces/Workflows.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class WorkflowsVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  element;
  constructor(options) {
    super(options);
    this.element = new WorkflowsElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'Workflow'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default WorkflowsVisitor;