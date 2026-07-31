import { MediaTypeElement } from '@swagger-api/apidom-ns-openapi-3-0';
/**
 * @public
 */
class MediaType extends MediaTypeElement {
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
export default MediaType;