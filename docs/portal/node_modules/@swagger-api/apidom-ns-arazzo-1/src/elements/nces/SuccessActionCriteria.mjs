import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SuccessActionCriteria extends ArrayElement {
  static primaryClass = 'success-action-criteria';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SuccessActionCriteria.primaryClass);
    this.classes.push('criteria');
  }
}
export default SuccessActionCriteria;