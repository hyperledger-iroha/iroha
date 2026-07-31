import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import RequestBodyElement from "../../../../elements/RequestBody.mjs";
/**
 * @public
 */
export const BaseRequestBodyVisitor = OpenApi3_1Specification.visitors.document.objects.RequestBody.$visitor;
/**
 * @public
 */
class RequestBodyVisitor extends BaseRequestBodyVisitor {
  constructor(options) {
    super(options);
    this.element = new RequestBodyElement();
  }
}
export default RequestBodyVisitor;