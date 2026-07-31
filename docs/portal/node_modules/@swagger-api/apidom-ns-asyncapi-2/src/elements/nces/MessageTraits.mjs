import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MessageTraits extends ArrayElement {
  static primaryClass = 'message-traits';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MessageTraits.primaryClass);
  }
}
export default MessageTraits;