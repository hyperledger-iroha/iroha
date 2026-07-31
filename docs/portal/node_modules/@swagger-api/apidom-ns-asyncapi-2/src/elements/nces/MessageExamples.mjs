import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MessageExamples extends ArrayElement {
  static primaryClass = 'message-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MessageExamples.primaryClass);
  }
}
export default MessageExamples;