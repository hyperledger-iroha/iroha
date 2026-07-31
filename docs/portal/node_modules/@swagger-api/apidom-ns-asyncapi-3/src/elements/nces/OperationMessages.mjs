import { ArrayElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class OperationMessages extends ArrayElement {
  static primaryClass = 'operation-messages';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationMessages.primaryClass);
  }
}
export default OperationMessages;