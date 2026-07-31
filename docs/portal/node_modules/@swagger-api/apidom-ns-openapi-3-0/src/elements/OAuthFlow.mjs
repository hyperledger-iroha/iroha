import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OAuthFlow extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'oAuthFlow';
  }
  get authorizationUrl() {
    return this.get('authorizationUrl');
  }
  set authorizationUrl(authorizationUrl) {
    this.set('authorizationUrl', authorizationUrl);
  }
  get tokenUrl() {
    return this.get('tokenUrl');
  }
  set tokenUrl(tokenUrl) {
    this.set('tokenUrl', tokenUrl);
  }
  get refreshUrl() {
    return this.get('refreshUrl');
  }
  set refreshUrl(refreshUrl) {
    this.set('refreshUrl', refreshUrl);
  }
  get scopes() {
    return this.get('scopes');
  }
  set scopes(scopes) {
    this.set('scopes', scopes);
  }
}
export default OAuthFlow;