import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SwaggerConsumes extends ArrayElement {
  static primaryClass = 'swagger-consumes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerConsumes.primaryClass);
  }
}
export default SwaggerConsumes;