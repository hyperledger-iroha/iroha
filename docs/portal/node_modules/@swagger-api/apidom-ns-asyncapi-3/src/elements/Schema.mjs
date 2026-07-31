import { SchemaElement } from '@swagger-api/apidom-ns-asyncapi-2';
/**
 * @public
 */
class Schema extends SchemaElement {
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
export default Schema;