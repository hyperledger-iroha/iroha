import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsOperations extends ObjectElement {
  static primaryClass = 'components-operations';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsOperations.primaryClass);
  }
}
export default ComponentsOperations;