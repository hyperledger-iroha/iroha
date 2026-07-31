import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import PathsElement from "../../../../elements/Paths.mjs";
/**
 * @public
 */
export const BasePathsVisitor = OpenApi3_1Specification.visitors.document.objects.Paths.$visitor;
/**
 * @public
 */
class PathsVisitor extends BasePathsVisitor {
  constructor(options) {
    super(options);
    this.element = new PathsElement();
  }
}
export default PathsVisitor;