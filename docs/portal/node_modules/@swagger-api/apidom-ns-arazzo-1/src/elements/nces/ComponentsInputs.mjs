import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsInputs extends ObjectElement {
  static primaryClass = 'components-inputs';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsInputs.primaryClass);
  }
}
export default ComponentsInputs;