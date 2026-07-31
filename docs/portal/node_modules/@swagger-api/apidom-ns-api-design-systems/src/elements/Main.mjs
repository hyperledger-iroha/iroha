import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class Main extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'main';
    this.classes.push('api');
  }
  get version() {
    return this.get('version');
  }
  set version(version) {
    this.set('version', version);
  }
  get info() {
    return this.get('info');
  }
  set info(info) {
    this.set('info', info);
  }
  get principles() {
    return this.get('principles');
  }
  set principles(principles) {
    this.set('principles', principles);
  }
  get standards() {
    return this.get('standards');
  }
  set standards(standards) {
    this.set('standards', standards);
  }
  get scenarios() {
    return this.get('scenarios');
  }
  set scenarios(scenarios) {
    this.set('scenarios', scenarios);
  }
}
export default Main;