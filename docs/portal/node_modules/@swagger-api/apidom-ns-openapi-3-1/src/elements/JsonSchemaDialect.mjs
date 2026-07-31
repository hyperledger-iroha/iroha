import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class JsonSchemaDialect extends StringElement {
  static default = new JsonSchemaDialect('https://spec.openapis.org/oas/3.1/dialect/base');
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'jsonSchemaDialect';
  }
}
export default JsonSchemaDialect;