import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Server extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'server';
  }
  get url() {
    return this.get('url');
  }
  set url(url) {
    this.set('url', url);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get variables() {
    return this.get('variables');
  }
  set variables(variables) {
    this.set('variables', variables);
  }
}
export default Server;