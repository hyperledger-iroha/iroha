import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import EncodingElement from "../../../../elements/Encoding.mjs";
/**
 * @public
 */
export const BaseEncodingVisitor = OpenApi3_1Specification.visitors.document.objects.Encoding.$visitor;
/**
 * @public
 */
class EncodingVisitor extends BaseEncodingVisitor {
  constructor(options) {
    super(options);
    this.element = new EncodingElement();
  }
}
export default EncodingVisitor;