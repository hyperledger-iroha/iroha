import { OperationElement } from '@swagger-api/apidom-ns-openapi-3-0';
/**
 * @public
 */
class Operation extends OperationElement {
  get requestBody() {
    return this.get('requestBody');
  }
  set requestBody(requestBody) {
    this.set('requestBody', requestBody);
  }
}
export default Operation;