import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import TagElement from "../../../../elements/Tag.mjs";
/**
 * @public
 */
export const BaseTagVisitor = OpenApi3_1Specification.visitors.document.objects.Tag.$visitor;
/**
 * @public
 */
class TagVisitor extends BaseTagVisitor {
  constructor(options) {
    super(options);
    this.element = new TagElement();
  }
}
export default TagVisitor;