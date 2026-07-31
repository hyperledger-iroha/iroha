import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class ComponentsReplies extends ObjectElement {
  static primaryClass = 'components-replies';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsReplies.primaryClass);
  }
}
export default ComponentsReplies;