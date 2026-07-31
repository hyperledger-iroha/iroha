import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class ComponentsExternalDocumentation extends ObjectElement {
  static primaryClass = 'components-external-documentation';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsExternalDocumentation.primaryClass);
  }
}
export default ComponentsExternalDocumentation;