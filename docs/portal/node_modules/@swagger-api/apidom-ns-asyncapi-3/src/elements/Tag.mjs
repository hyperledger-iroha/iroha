import { TagElement } from '@swagger-api/apidom-ns-asyncapi-2';
/**
 * @public
 */
class Tag extends TagElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'tag';
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
export default Tag;