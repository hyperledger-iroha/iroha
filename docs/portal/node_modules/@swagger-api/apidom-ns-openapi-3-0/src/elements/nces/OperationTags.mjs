import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class OperationTags extends ArrayElement {
  static primaryClass = 'operation-tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationTags.primaryClass);
  }
}
export default OperationTags;