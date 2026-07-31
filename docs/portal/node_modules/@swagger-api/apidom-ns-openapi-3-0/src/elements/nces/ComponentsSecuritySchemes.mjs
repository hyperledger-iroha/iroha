import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsSecuritySchemes extends ObjectElement {
  static primaryClass = 'components-security-schemes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsSecuritySchemes.primaryClass);
  }
}
export default ComponentsSecuritySchemes;