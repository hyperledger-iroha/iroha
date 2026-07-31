import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsOperationBindings extends ObjectElement {
  static primaryClass = 'components-operation-bindings';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsOperationBindings.primaryClass);
  }
}
export default ComponentsOperationBindings;