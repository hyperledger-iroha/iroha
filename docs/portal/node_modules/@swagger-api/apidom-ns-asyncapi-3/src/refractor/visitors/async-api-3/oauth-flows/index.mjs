import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import OAuthFlowsElement from "../../../../elements/OauthFlows.mjs";
/**
 * @public
 */
export const BaseOAuthFlowsVisitor = AsyncApi2Specification.visitors.document.objects.OAuthFlows.$visitor;
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