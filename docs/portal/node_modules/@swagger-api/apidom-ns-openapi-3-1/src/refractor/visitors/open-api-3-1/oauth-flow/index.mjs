import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import OAuthFlowElement from "../../../../elements/OAuthFlow.mjs";
/**
 * @public
 */
export const BaseOAuthFlowVisitor = OpenApi3_1Specification.visitors.document.objects.OAuthFlow.$visitor;
/**
 * @public
 */
class OAuthFlowVisitor extends BaseOAuthFlowVisitor {
  constructor(options) {
    super(options);
    this.element = new OAuthFlowElement();
  }
}
export default OAuthFlowVisitor;