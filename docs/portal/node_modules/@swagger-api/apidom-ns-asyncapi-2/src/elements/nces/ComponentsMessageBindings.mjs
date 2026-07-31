import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsMessageBindings extends ObjectElement {
  static primaryClass = 'components-message-bindings';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsMessageBindings.primaryClass);
  }
}
export default ComponentsMessageBindings;