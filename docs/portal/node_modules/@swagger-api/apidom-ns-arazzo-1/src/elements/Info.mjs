import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Info extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'info';
    this.classes.push('info');
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get version() {
    return this.get('version');
  }
  set version(version) {
    this.set('version', version);
  }
}
export default Info;