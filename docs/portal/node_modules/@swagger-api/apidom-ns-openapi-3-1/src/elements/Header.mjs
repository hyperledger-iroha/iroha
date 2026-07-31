import { HeaderElement } from '@swagger-api/apidom-ns-openapi-3-0';
/**
 * @public
 */
class Header extends HeaderElement {
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
export default Header;