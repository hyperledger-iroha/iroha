import { SecuritySchemeElement } from '@swagger-api/apidom-ns-asyncapi-2';
/**
 * @public
 */
class SecurityScheme extends SecuritySchemeElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'securityScheme';
  }
  get scopes() {
    return this.get('scopes');
  }
  set scopes(scopes) {
    this.set('scopes', scopes);
  }
}
export default SecurityScheme;