import { InfoElement } from '@swagger-api/apidom-ns-asyncapi-2';
/**
 * @public
 */
class Info extends InfoElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'info';
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
export default Info;