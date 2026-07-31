import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsHeaders extends ObjectElement {
  static primaryClass = 'components-headers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsHeaders.primaryClass);
  }
}
export default ComponentsHeaders;