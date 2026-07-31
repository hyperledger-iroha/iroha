import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsParameters extends ObjectElement {
  static primaryClass = 'components-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsParameters.primaryClass);
    this.classes.push('parameters');
  }
}
export default ComponentsParameters;