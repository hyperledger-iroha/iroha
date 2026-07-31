import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ParametersElement from "../../../../elements/Parameters.mjs";
export const BaseParametersVisitor = AsyncApi2Specification.visitors.document.objects.Parameters.$visitor;
/**
 * @public
 */
class ParametersVisitor extends BaseParametersVisitor {
  constructor(options) {
    super(options);
    this.element = new ParametersElement();
  }
}
export default ParametersVisitor;