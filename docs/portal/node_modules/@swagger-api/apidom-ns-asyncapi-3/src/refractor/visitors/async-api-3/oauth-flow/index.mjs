import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import OAuthFlowElement from "../../../../elements/OauthFlow.mjs";
/**
 * @public
 */
export const BaseOAuthFlowVisitor = AsyncApi2Specification.visitors.document.objects.OAuthFlow.$visitor;
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