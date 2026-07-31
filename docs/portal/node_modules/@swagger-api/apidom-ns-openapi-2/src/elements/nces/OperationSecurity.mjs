import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationSecurity extends ArrayElement {
  static primaryClass = 'operation-security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationSecurity.primaryClass);
    this.classes.push('security');
  }
}
export default OperationSecurity;