import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsServerBindings extends ObjectElement {
  static primaryClass = 'components-server-bindings';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsServerBindings.primaryClass);
  }
}
export default ComponentsServerBindings;