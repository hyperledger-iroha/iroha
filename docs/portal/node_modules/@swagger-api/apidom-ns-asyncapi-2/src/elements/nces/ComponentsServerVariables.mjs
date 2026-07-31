import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsServerVariables extends ObjectElement {
  static primaryClass = 'components-server-variables';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsServerVariables.primaryClass);
  }
}
export default ComponentsServerVariables;