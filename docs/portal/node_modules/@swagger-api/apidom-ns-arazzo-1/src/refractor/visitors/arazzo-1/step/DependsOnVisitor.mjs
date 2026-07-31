import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import StepDependsOnElement from "../../../../elements/nces/StepDependsOn.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class DependsOnVisitor extends FallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new StepDependsOnElement();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat(cloneDeep(arrayElement));
    return BREAK;
  }
}
export default DependsOnVisitor;