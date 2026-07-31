import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import OperationProducesElement from "../../../../elements/nces/OperationProduces.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class ProducesVisitor extends FallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationProducesElement();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat(cloneDeep(arrayElement));
    return BREAK;
  }
}
export default ProducesVisitor;