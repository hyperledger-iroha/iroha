import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsPathItems extends ObjectElement {
  static primaryClass = 'components-path-items';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsPathItems.primaryClass);
  }
}
export default ComponentsPathItems;