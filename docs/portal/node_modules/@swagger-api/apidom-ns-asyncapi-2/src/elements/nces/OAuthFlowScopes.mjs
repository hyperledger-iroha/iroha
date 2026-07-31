import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OAuthFlowScopes extends ObjectElement {
  static primaryClass = 'oauth-flow-scopes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OAuthFlowScopes.primaryClass);
  }
}
export default OAuthFlowScopes;