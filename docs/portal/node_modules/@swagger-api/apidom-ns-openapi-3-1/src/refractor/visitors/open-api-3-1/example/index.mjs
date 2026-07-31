import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ExampleElement from "../../../../elements/Example.mjs";
/**
 * @public
 */
export const BaseExampleVisitor = OpenApi3_1Specification.visitors.document.objects.Example.$visitor;
/**
 * @public
 */
class ExampleVisitor extends BaseExampleVisitor {
  constructor(options) {
    super(options);
    this.element = new ExampleElement();
  }
}
export default ExampleVisitor;