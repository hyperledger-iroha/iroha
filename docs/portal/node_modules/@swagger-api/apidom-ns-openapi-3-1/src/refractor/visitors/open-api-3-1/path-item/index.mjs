import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import PathItemElement from "../../../../elements/PathItem.mjs";
/**
 * @public
 */
export const BasePathItemVisitor = OpenApi3_1Specification.visitors.document.objects.PathItem.$visitor;
/**
 * @public
 */
class PathItemVisitor extends BasePathItemVisitor {
  constructor(options) {
    super(options);
    this.element = new PathItemElement();
  }
}
export default PathItemVisitor;