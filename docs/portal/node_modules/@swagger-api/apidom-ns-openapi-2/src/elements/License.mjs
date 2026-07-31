import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class License extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'license';
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get url() {
    return this.get('url');
  }
  set url(url) {
    this.set('url', url);
  }
}
export default License;