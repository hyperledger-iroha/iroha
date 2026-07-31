import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsLinks extends ObjectElement {
  static primaryClass = 'components-links';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsLinks.primaryClass);
  }
}
export default ComponentsLinks;