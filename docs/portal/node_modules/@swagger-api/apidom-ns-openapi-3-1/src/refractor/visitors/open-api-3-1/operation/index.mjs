import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import OperationElement from "../../../../elements/Operation.mjs";
/**
 * @public
 */
export const BaseOperationVisitor = OpenApi3_1Specification.visitors.document.objects.Operation.$visitor;
/**
 * @public
 */
class OperationVisitor extends BaseOperationVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationElement();
  }
}
export default OperationVisitor;