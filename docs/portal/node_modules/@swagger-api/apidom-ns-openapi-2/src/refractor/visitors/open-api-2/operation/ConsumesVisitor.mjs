import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import OperationConsumesElement from "../../../../elements/nces/OperationConsumes.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class ConsumesVisitor extends FallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationConsumesElement();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat(cloneDeep(arrayElement));
    return BREAK;
  }
}
export default ConsumesVisitor;