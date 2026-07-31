import { MediaTypes } from '@swagger-api/apidom-core';

/**
 * @public
 */
export class JSONMediaTypes extends MediaTypes {
  latest() {
    return this[0];
  }
}

/**
 * @public
 */
const mediaTypes = new JSONMediaTypes('application/json');
export default mediaTypes;