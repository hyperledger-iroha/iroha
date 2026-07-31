import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class WebSocketChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'webSocketChannelBinding';
    this.classes.push('channel-binding');
  }
  get method() {
    return this.get('method');
  }
  set method(method) {
    this.set('method', method);
  }
  get query() {
    return this.get('query');
  }
  set query(query) {
    this.set('query', query);
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
export default WebSocketChannelBinding;