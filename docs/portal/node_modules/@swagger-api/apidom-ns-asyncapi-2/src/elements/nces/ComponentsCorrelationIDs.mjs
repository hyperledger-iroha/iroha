import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsCorrelationIDs extends ObjectElement {
  static primaryClass = 'components-correlation-ids';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsCorrelationIDs.primaryClass);
  }
}
export default ComponentsCorrelationIDs;