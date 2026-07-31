import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MessageTraitExamples extends ArrayElement {
  static primaryClass = 'message-trait-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MessageTraitExamples.primaryClass);
  }
}
export default MessageTraitExamples;