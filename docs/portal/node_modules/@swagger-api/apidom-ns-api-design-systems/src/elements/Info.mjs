import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Info extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'info';
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
}
export default Info;