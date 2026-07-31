import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import OperationBindingsElement from "../../../../elements/OperationBindings.mjs";
export const BaseOperationBindingsVisitor = AsyncApi2Specification.visitors.document.objects.OperationBindings.$visitor;
/**
 * @public
 */
class OperationBindingsVisitor extends BaseOperationBindingsVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationBindingsElement();
  }
}
export default OperationBindingsVisitor;