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
  get scheme() {
    return this.get('scheme');
  }
  set scheme(scheme) {
    this.set('scheme', scheme);
  }
  get bearerFormat() {
    return this.get('bearerFormat');
  }
  set bearerFormat(bearerFormat) {
    this.set('bearerFormat', bearerFormat);
  }
  get flows() {
    return this.get('flows');
  }
  set flows(flows) {
    this.set('flows', flows);
  }
  get openIdConnectUrl() {
    return this.get('openIdConnectUrl');
  }
  set openIdConnectUrl(openIdConnectUrl) {
    this.set('openIdConnectUrl', openIdConnectUrl);
  }
}
export default SecurityScheme;