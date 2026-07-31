import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Criterion extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'criterion';
  }
  get context() {
    return this.get('context');
  }
  set context(context) {
    this.set('context', context);
  }
  get condition() {
    return this.get('condition');
  }
  set condition(condition) {
    this.set('condition', condition);
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
}
export default Criterion;