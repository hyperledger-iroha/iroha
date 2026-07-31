import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { OAuthFlowElement } from '@swagger-api/apidom-ns-asyncapi-2';

/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class OAuthFlow extends OAuthFlowElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'oAuthFlow';
  }
  get scopes() {
    throw new UnsupportedOperationError('scopes keyword from Core vocabulary has been renamed to availableScopes.');
  }
  set scopes(scopes) {
    throw new UnsupportedOperationError('scopes keyword from Core vocabulary has been renamed to availableScopes.');
  }
  get availableScopes() {
    return this.get('availableScopes');
  }
  set availableScopes(availableScopes) {
    this.set('availableScopes', availableScopes);
  }
}
export default OAuthFlow;