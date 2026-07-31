import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ComponentsElement from "../../../../elements/Components.mjs";
/**
 * @public
 */
export const BaseComponentsVisitor = OpenApi3_1Specification.visitors.document.objects.Components.$visitor;
/**
 * @public
 */
class ComponentsVisitor extends BaseComponentsVisitor {
  constructor(options) {
    super(options);
    this.element = new ComponentsElement();
  }
}
export default ComponentsVisitor;