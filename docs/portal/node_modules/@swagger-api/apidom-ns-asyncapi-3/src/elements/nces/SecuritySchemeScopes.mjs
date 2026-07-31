import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SecuritySchemeScopes extends ArrayElement {
  static primaryClass = 'security-scheme-scopes';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SecuritySchemeScopes.primaryClass);
  }
}
export default SecuritySchemeScopes;