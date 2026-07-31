import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class MediaType extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mediaType';
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
  get example() {
    return this.get('example');
  }
  set example(example) {
    this.set('example', example);
  }
  get examples() {
    return this.get('examples');
  }
  set examples(examples) {
    this.set('examples', examples);
  }
  get encoding() {
    return this.get('encoding');
  }
  set encoding(encoding) {
    this.set('encoding', encoding);
  }
}
export default MediaType;