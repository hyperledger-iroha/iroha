"use strict";

exports.__esModule = true;
exports.default = exports.AsyncAPIMediaTypes = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
class AsyncAPIMediaTypes extends _apidomCore.MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'asyncapi;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = '3.0.0', format = 'generic') {
    const search = format === 'generic' ? `vnd.aai.asyncapi;version=${version}` : `vnd.aai.asyncapi+${format};version=${version}`;
    const found = this.find(mediaType => mediaType.includes(search));
    return found || this.unknownMediaType;
  }
  latest(format = 'generic') {
    return (0, _ramda.last)(this.filterByFormat(format));
  }
}

/**
 * @public
 */
exports.AsyncAPIMediaTypes = AsyncAPIMediaTypes;
const mediaTypes = new AsyncAPIMediaTypes('application/vnd.aai.asyncapi;version=3.0.0', 'application/vnd.aai.asyncapi+json;version=3.0.0', 'application/vnd.aai.asyncapi+yaml;version=3.0.0');
var _default = exports.default = mediaTypes;