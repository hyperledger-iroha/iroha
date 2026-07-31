import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import HeaderElement from "../../../../elements/Header.mjs";
/**
 * @public
 */
export const BaseHeaderVisitor = OpenApi3_1Specification.visitors.document.objects.Header.$visitor;
/**
 * @public
 */
class HeaderVisitor extends BaseHeaderVisitor {
  constructor(options) {
    super(options);
    this.element = new HeaderElement();
  }
}
export default HeaderVisitor;