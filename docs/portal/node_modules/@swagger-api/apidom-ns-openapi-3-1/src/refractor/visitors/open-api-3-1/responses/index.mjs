import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ResponsesElement from "../../../../elements/Responses.mjs";
/**
 * @public
 */
export const BaseResponsesVisitor = OpenApi3_1Specification.visitors.document.objects.Responses.$visitor;
/**
 * @public
 */
class ResponsesVisitor extends BaseResponsesVisitor {
  constructor(options) {
    super(options);
    this.element = new ResponsesElement();
  }
}
export default ResponsesVisitor;