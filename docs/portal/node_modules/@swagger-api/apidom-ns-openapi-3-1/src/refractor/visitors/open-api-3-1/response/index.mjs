import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ResponseElement from "../../../../elements/Response.mjs";
/**
 * @public
 */
export const BaseResponseVisitor = OpenApi3_1Specification.visitors.document.objects.Response.$visitor;
/**
 * @public
 */
class ResponseVisitor extends BaseResponseVisitor {
  constructor(options) {
    super(options);
    this.element = new ResponseElement();
  }
}
export default ResponseVisitor;