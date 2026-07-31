import { OperationTraitElement } from '@swagger-api/apidom-ns-asyncapi-2';
import { UnsupportedOperationError } from '@swagger-api/apidom-error';
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class OperationTrait extends OperationTraitElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationTrait';
  }
  get operationId() {
    throw new UnsupportedOperationError('operationId keyword from Core vocabulary has been removed');
  }
  set operationId(operationId) {
    throw new UnsupportedOperationError('operationId keyword from Core vocabulary has been removed');
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
export default OperationTrait;