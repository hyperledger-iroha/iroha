import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsCallbacks extends ObjectElement {
  static primaryClass = 'components-callbacks';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsCallbacks.primaryClass);
  }
}
export default ComponentsCallbacks;