import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ReferenceElement from "../../../../elements/Reference.mjs";
/**
 * @public
 */
export const BaseReferenceVisitor = OpenApi3_1Specification.visitors.document.objects.Reference.$visitor;
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