import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class ComponentsTags extends ObjectElement {
  static primaryClass = 'components-tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsTags.primaryClass);
  }
}
export default ComponentsTags;