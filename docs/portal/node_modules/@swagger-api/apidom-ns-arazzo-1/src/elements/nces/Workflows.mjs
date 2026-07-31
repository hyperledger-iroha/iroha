import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Workflows extends ArrayElement {
  static primaryClass = 'workflows';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Workflows.primaryClass);
  }
}
export default Workflows;