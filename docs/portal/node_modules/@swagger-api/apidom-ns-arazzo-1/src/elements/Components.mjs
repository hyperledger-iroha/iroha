import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Components extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'components';
  }
  get inputs() {
    return this.get('inputs');
  }
  set inputs(inputs) {
    this.set('inputs', inputs);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
}
export default Components;