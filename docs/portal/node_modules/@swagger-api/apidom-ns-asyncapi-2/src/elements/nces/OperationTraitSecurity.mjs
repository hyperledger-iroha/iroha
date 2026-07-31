import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationTraitSecurity extends ArrayElement {
  static primaryClass = 'operation-trait-security';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationTraitSecurity.primaryClass);
  }
}
export default OperationTraitSecurity;