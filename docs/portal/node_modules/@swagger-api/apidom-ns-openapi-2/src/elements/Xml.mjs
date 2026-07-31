import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Xml extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'xml';
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get namespace() {
    return this.get('namespace');
  }
  set namespace(namespace) {
    this.set('namespace', namespace);
  }
  get prefix() {
    return this.get('prefix');
  }
  set prefix(prefix) {
    this.set('prefix', prefix);
  }
  get attribute() {
    return this.get('attribute');
  }
  set attribute(attribute) {
    this.set('attribute', attribute);
  }
  get wrapped() {
    return this.get('wrapped');
  }
  set wrapped(wrapped) {
    this.set('wrapped', wrapped);
  }
}
export default Xml;