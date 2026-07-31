import { last } from 'ramda';
import { MediaTypes } from '@swagger-api/apidom-core';

/**
 * @public
 */

/**
 * @public
 */
export class JSONSchemaDraft6MediaTypes extends MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'schema;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = 'draft-06', format = 'generic') {
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
const mediaTypes = new JSONSchemaDraft6MediaTypes('application/schema;version=draft-06', 'application/schema+json;version=draft-06', 'application/schema+yaml;version=draft-06');
export default mediaTypes;