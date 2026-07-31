import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import OAuthFlowsElement from "../../../../elements/OAuthFlows.mjs";
/**
 * @public
 */
export const BaseOAuthFlowsVisitor = OpenApi3_1Specification.visitors.document.objects.OAuthFlows.$visitor;
/**
 * @public
 */
class OAuthFlowsVisitor extends BaseOAuthFlowsVisitor {
  constructor(options) {
    super(options);
    this.element = new OAuthFlowsElement();
  }
}
export default OAuthFlowsVisitor;