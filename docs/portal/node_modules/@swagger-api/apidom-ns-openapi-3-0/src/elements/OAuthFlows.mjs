import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class OAuthFlows extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'oAuthFlows';
  }
  get implicit() {
    return this.get('implicit');
  }
  set implicit(implicit) {
    this.set('implicit', implicit);
  }
  get password() {
    return this.get('password');
  }
  set password(password) {
    this.set('password', password);
  }
  get clientCredentials() {
    return this.get('clientCredentials');
  }
  set clientCredentials(clientCredentials) {
    this.set('clientCredentials', clientCredentials);
  }
  get authorizationCode() {
    return this.get('authorizationCode');
  }
  set authorizationCode(authorizationCode) {
    this.set('authorizationCode', authorizationCode);
  }
}
export default OAuthFlows;