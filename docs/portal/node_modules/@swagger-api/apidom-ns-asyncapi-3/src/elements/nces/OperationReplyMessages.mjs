import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationReplyMessages extends ArrayElement {
  static primaryClass = 'operation-reply-messages';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationReplyMessages.primaryClass);
  }
}
export default OperationReplyMessages;