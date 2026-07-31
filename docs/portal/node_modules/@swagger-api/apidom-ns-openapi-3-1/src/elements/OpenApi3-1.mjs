import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
class OpenApi3_1 extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'openApi3_1';
    this.classes.push('api');
  }
  get openapi() {
    return this.get('openapi');
  }
  set openapi(openapi) {
    this.set('openapi', openapi);
  }
  get info() {
    return this.get('info');
  }
  set info(info) {
    this.set('info', info);
  }
  get jsonSchemaDialect() {
    return this.get('jsonSchemaDialect');
  }
  set jsonSchemaDialect(jsonSchemaDialect) {
    this.set('jsonSchemaDialect', jsonSchemaDialect);
  }
  get servers() {
    return this.get('servers');
  }
  set servers(servers) {
    this.set('servers', servers);
  }
  get paths() {
    return this.get('paths');
  }
  set paths(paths) {
    this.set('paths', paths);
  }
  get components() {
    return this.get('components');
  }
  set components(components) {
    this.set('components', components);
  }
  get security() {
    return this.get('security');
  }
  set security(security) {
    this.set('security', security);
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
  get webhooks() {
    return this.get('webhooks');
  }
  set webhooks(webhooks) {
    this.set('webhooks', webhooks);
  }
}
export default OpenApi3_1;