import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import CorrelationIDElement from "../../../../elements/CorrelationID.mjs";
export const BaseCorrelationIDVisitor = AsyncApi2Specification.visitors.document.objects.CorrelationID.$visitor;
/**
 * @public
 */
class CorrelationIDVisitor extends BaseCorrelationIDVisitor {
  constructor(options) {
    super(options);
    this.element = new CorrelationIDElement();
  }
}
export default CorrelationIDVisitor;