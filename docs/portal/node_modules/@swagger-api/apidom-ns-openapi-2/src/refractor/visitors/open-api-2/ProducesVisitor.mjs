import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import SwaggerProducesElement from "../../../elements/nces/SwaggerProduces.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
/**
 * @public
 */
class ProducesVisitor extends FallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new SwaggerProducesElement();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat(cloneDeep(arrayElement));
    return BREAK;
  }
}
export default ProducesVisitor;