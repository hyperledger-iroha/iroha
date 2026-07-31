import { last } from 'ramda';
import { MediaTypes } from '@swagger-api/apidom-core';

/**
 * @public
 */

/**
 * @public
 */
export class ApiDesignSystemsMediaTypes extends MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'apidesignsystems;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = '2021-05-07', format = 'generic') {
    const search = format === 'generic' ? `apidesignsystems;version=${version}` : `apidesignsystems+${format};version=${version}`;
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
const mediaTypes = new ApiDesignSystemsMediaTypes('application/vnd.aai.apidesignsystems;version=2021-05-07', 'application/vnd.aai.apidesignsystems+json;version=2021-05-07', 'application/vnd.aai.apidesignsystems+yaml;version=2021-05-07');
export default mediaTypes;