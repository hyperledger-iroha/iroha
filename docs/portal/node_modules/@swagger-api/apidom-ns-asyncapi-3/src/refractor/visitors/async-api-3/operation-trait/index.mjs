import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import OperationTraitElement from "../../../../elements/OperationTrait.mjs";
export const BaseOperationTraitVisitor = AsyncApi2Specification.visitors.document.objects.OperationTrait.$visitor;
/**
 * @public
 */
class OperationTraitVisitor extends BaseOperationTraitVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationTraitElement();
  }
}
export default OperationTraitVisitor;