import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsMessageTraits extends ObjectElement {
  static primaryClass = 'components-message-traits';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsMessageTraits.primaryClass);
  }
}
export default ComponentsMessageTraits;