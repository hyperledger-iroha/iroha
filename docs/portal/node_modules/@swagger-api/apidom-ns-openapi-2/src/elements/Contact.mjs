import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Contact extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'contact';
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
  get email() {
    return this.get('email');
  }
  set email(email) {
    this.set('email', email);
  }
}
export default Contact;