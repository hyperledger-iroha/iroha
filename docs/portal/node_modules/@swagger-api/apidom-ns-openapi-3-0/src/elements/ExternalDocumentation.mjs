import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ExternalDocumentation extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'externalDocumentation';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get url() {
    return this.get('url');
  }
  set url(url) {
    this.set('url', url);
  }
}
export default ExternalDocumentation;