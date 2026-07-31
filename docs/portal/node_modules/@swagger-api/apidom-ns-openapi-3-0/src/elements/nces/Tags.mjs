import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Tags extends ArrayElement {
  static primaryClass = 'tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Tags.primaryClass);
  }
}
export default Tags;