import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class Parameter extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'parameter';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
  get location() {
    return this.get('location');
  }
  set location(location) {
    this.set('location', location);
  }
}
export default Parameter;