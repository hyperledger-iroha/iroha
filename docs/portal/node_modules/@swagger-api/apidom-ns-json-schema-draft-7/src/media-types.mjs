import { last } from 'ramda';
import { MediaTypes } from '@swagger-api/apidom-core';

/**
 * @public
 */

/**
 * @public
 */
export class JSONSchemaDraft7MediaTypes extends MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'schema;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = 'draft-07', format = 'generic') {
    const search = format === 'generic' ? `schema;version=${version}` : `schema+${format};version=${version}`;
    const found = this.find(mediaType => mediaType.includes(search));
    return found || this.unknownMediaType;
  }
  latest(format = 'generic') {
    return last(this.filterByFormat(format));
  }
}

/**
 * @public
 */
const mediaTypes = new JSONSchemaDraft7MediaTypes('application/schema;version=draft-07', 'application/schema+json;version=draft-07', 'application/schema+yaml;version=draft-07');
export default mediaTypes;