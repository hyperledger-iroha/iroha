import { ComponentsElement } from '@swagger-api/apidom-ns-openapi-3-0';

/**
 * @public
 */
class Components extends ComponentsElement {
  get pathItems() {
    return this.get('pathItems');
  }
  set pathItems(pathItems) {
    this.set('pathItems', pathItems);
  }
}
export default Components;