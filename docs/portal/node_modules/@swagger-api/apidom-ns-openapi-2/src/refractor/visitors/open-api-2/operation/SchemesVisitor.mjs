import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import OperationSchemesElement from "../../../../elements/nces/OperationSchemes.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class SchemesVisitor extends FallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationSchemesElement();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat(cloneDeep(arrayElement));
    return BREAK;
  }
}
export default SchemesVisitor;