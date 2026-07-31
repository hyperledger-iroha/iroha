import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsRequestBodies extends ObjectElement {
  static primaryClass = 'components-request-bodies';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsRequestBodies.primaryClass);
  }
}
export default ComponentsRequestBodies;