"use strict";

exports.__esModule = true;
exports.default = exports.ArazzoMediaTypes = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
class ArazzoMediaTypes extends _apidomCore.MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'workflows;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = '1.0.1', format = 'generic') {
    const search = format === 'generic' ? `vnd.oai.workflows;version=${version}` : `vnd.oai.workflows+${format};version=${version}`;
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
exports.ArazzoMediaTypes = ArazzoMediaTypes;
const mediaTypes = new ArazzoMediaTypes('application/vnd.oai.workflows;version=1.0.0', 'application/vnd.oai.workflows+json;version=1.0.0', 'application/vnd.oai.workflows+yaml;version=1.0.0', 'application/vnd.oai.workflows;version=1.0.1', 'application/vnd.oai.workflows+json;version=1.0.1', 'application/vnd.oai.workflows+yaml;version=1.0.1');
var _default = exports.default = mediaTypes;