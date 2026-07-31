import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsSchemas extends ObjectElement {
  static primaryClass = 'components-schemas';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsSchemas.primaryClass);
  }
}
export default ComponentsSchemas;