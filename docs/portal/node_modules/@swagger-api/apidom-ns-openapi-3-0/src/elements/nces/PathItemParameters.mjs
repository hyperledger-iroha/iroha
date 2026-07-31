import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class PathItemParameters extends ArrayElement {
  static primaryClass = 'path-item-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(PathItemParameters.primaryClass);
    this.classes.push('parameters');
  }
}
export default PathItemParameters;