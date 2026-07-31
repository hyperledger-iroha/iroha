import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SecurityScheme extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'securityScheme';
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get in() {
    return this.get('in');
  }
  set in(inVal) {
    this.set('in', inVal);
  }
  get flow() {
    return this.get('flow');
  }
  set flow(flow) {
    this.set('flow', flow);
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
  get scopes() {
    return this.get('scopes');
  }
  set scopes(scopes) {
    this.set('scopes', scopes);
  }
}
export default SecurityScheme;