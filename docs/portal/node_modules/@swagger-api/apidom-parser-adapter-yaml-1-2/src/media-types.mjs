import { MediaTypes } from '@swagger-api/apidom-core';

/**
 * @public
 */
export class YamlMediaTypes extends MediaTypes {
  latest() {
    return this[1];
  }
}

/**
 * @public
 */
const mediaTypes = new YamlMediaTypes('text/yaml', 'application/yaml');
export default mediaTypes;