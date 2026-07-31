import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class AsyncApi2 extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'asyncApi2';
    this.classes.push('api');
  }
  get asyncapi() {
    return this.get('asyncapi');
  }
  set asyncapi(asyncapi) {
    this.set('asyncapi', asyncapi);
  }
  get idProp() {
    return this.get('id');
  }
  set idProp(id) {
    this.set('id', id);
  }
  get info() {
    return this.get('info');
  }
  set info(info) {
    this.set('info', info);
  }
  get servers() {
    return this.get('servers');
  }
  set servers(servers) {
    this.set('servers', servers);
  }
  get defaultContentType() {
    return this.get('defaultContentType');
  }
  set defaultContentType(defaultContentType) {
    this.set('defaultContentType', defaultContentType);
  }
  get channels() {
    return this.get('channels');
  }
  set channels(channels) {
    this.set('channels', channels);
  }
  get components() {
    return this.get('components');
  }
  set components(components) {
    this.set('components', components);
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
}
export default AsyncApi2;