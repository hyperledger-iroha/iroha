import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ReferenceElement from "../../../../elements/Reference.mjs";
/**
 * @public
 */
export const BaseReferenceVisitor = AsyncApi2Specification.visitors.document.objects.Reference.$visitor;
/**
 * @public
 */
class ReferenceVisitor extends BaseReferenceVisitor {
  constructor(options) {
    super(options);
    this.element = new ReferenceElement();
  }
}
export default ReferenceVisitor;