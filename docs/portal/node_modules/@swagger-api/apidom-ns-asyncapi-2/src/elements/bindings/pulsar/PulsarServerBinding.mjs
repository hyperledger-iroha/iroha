import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class PulsarServerBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'pulsarServerBinding';
    this.classes.push('server-binding');
  }
  get tenant() {
    return this.get('tenant');
  }
  set tenant(tenant) {
    this.set('tenant', tenant);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
export default PulsarServerBinding;