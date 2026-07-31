import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class IbmmqMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'ibmmqMessageBinding';
    this.classes.push('message-binding');
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get expiry() {
    return this.get('expiry');
  }
  set expiry(expiry) {
    this.set('expiry', expiry);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
export default IbmmqMessageBinding;