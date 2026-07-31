import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import LinkElement from "../../../../elements/Link.mjs";
/**
 * @public
 */
export const BaseLinkVisitor = OpenApi3_1Specification.visitors.document.objects.Link.$visitor;
/**
 * @public
 */
class LinkVisitor extends BaseLinkVisitor {
  constructor(options) {
    super(options);
    this.element = new LinkElement();
  }
}
export default LinkVisitor;