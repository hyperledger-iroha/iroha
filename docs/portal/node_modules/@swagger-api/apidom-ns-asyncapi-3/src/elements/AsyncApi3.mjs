import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { AsyncApi2Element } from '@swagger-api/apidom-ns-asyncapi-2';
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class AsyncApi3 extends AsyncApi2Element {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'asyncApi3';
  }
  get tags() {
    throw new UnsupportedOperationError('tags keyword has been moved to info');
  }
  set tags(tags) {
    throw new UnsupportedOperationError('tags keyword has been moved to info');
  }
  get externalDocs() {
    throw new UnsupportedOperationError('externalDocs keyword has been moved to info.');
  }
  set externalDocs(externalDocs) {
    throw new UnsupportedOperationError('externalDocs keyword has been moved to info.');
  }
  get operations() {
    return this.get('operations');
  }
  set operations(operations) {
    this.set('operations', operations);
  }
}
export default AsyncApi3;