import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SwaggerSchemes extends ArrayElement {
  static primaryClass = 'swagger-schemes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SwaggerSchemes.primaryClass);
  }
}
export default SwaggerSchemes;