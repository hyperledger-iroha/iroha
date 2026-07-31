import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SwaggerTags extends ArrayElement {
  static primaryClass = 'swagger-tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerTags.primaryClass);
  }
}
export default SwaggerTags;