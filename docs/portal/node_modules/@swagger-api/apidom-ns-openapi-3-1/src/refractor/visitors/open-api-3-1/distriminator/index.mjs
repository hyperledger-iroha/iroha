import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import DiscriminatorElement from "../../../../elements/Discriminator.mjs";
/**
 * @public
 */
export const BaseDiscriminatorVisitor = OpenApi3_1Specification.visitors.document.objects.Discriminator.$visitor;
/**
 * @public
 */
class DiscriminatorVisitor extends BaseDiscriminatorVisitor {
  constructor(options) {
    super(options);
    this.element = new DiscriminatorElement();
    this.canSupportSpecificationExtensions = true;
  }
}
export default DiscriminatorVisitor;