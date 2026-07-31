import { InfoElement } from '@swagger-api/apidom-ns-openapi-3-0';
/**
 * @public
 */
class Info extends InfoElement {
  get license() {
    return this.get('license');
  }
  set license(licenseElement) {
    this.set('license', licenseElement);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
}
export default Info;