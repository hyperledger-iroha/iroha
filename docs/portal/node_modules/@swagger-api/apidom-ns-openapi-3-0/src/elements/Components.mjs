import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Components extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'components';
  }
  get schemas() {
    return this.get('schemas');
  }
  set schemas(schemas) {
    this.set('schemas', schemas);
  }
  get responses() {
    return this.get('responses');
  }
  set responses(responses) {
    this.set('responses', responses);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
  get examples() {
    return this.get('examples');
  }
  set examples(examples) {
    this.set('examples', examples);
  }
  get requestBodies() {
    return this.get('requestBodies');
  }
  set requestBodies(requestBodies) {
    this.set('requestBodies', requestBodies);
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get securitySchemes() {
    return this.get('securitySchemes');
  }
  set securitySchemes(securitySchemes) {
    this.set('securitySchemes', securitySchemes);
  }
  get links() {
    return this.get('links');
  }
  set links(links) {
    this.set('links', links);
  }
  get callbacks() {
    return this.get('callbacks');
  }
  set callbacks(callbacks) {
    this.set('callbacks', callbacks);
  }
}
export default Components;