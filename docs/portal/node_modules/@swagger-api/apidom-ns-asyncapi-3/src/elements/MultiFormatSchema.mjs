import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MultiFormatSchema extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'multiFormatSchema';
  }
  get schemaFormat() {
    return this.get('schemaFormat');
  }
  set schemaFormat(schemaFormat) {
    this.set('schemaFormat', schemaFormat);
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
export default MultiFormatSchema;