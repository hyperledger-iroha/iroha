import { ParameterElement } from '@swagger-api/apidom-ns-openapi-3-0';
/**
 * @public
 */
class Parameter extends ParameterElement {
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
export default Parameter;