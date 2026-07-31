import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ServerVariable extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'serverVariable';
  }
  get enum() {
    return this.get('enum');
  }
  set enum(value) {
    this.set('enum', value);
  }
  get default() {
    return this.get('default');
  }
  set default(value) {
    this.set('default', value);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
}
export default ServerVariable;