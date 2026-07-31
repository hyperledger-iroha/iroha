import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ComponentsElement from "../../../../elements/Components.mjs";
export const BaseComponentsVisitor = AsyncApi2Specification.visitors.document.objects.Components.$visitor;
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