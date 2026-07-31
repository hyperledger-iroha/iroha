import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class Principle extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'principle';
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get iri() {
    return this.get('iri');
  }
  set iri(iri) {
    this.set('iri', iri);
  }
  get level() {
    return this.get('level');
  }
  set level(level) {
    this.set('level', level);
  }
}
export default Principle;