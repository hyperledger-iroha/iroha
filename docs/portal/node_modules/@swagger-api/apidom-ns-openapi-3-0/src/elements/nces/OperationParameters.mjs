import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationParameters extends ArrayElement {
  static primaryClass = 'operation-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationParameters.primaryClass);
    this.classes.push('parameters');
  }
}
export default OperationParameters;