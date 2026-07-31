"use strict";

exports.__esModule = true;
exports.default = exports.OpenAPIMediaTypes = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
class OpenAPIMediaTypes extends _apidomCore.MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'openapi;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = '3.0.4', format = 'generic') {
    const search = format === 'generic' ? `vnd.oai.openapi;version=${version}` : `vnd.oai.openapi+${format};version=${version}`;
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
exports.OpenAPIMediaTypes = OpenAPIMediaTypes;
const mediaTypes = new OpenAPIMediaTypes('application/vnd.oai.openapi;version=3.0.0', 'application/vnd.oai.openapi+json;version=3.0.0', 'application/vnd.oai.openapi+yaml;version=3.0.0', 'application/vnd.oai.openapi;version=3.0.1', 'application/vnd.oai.openapi+json;version=3.0.1', 'application/vnd.oai.openapi+yaml;version=3.0.1', 'application/vnd.oai.openapi;version=3.0.2', 'application/vnd.oai.openapi+json;version=3.0.2', 'application/vnd.oai.openapi+yaml;version=3.0.2', 'application/vnd.oai.openapi;version=3.0.3', 'application/vnd.oai.openapi+json;version=3.0.3', 'application/vnd.oai.openapi+yaml;version=3.0.3', 'application/vnd.oai.openapi;version=3.0.4', 'application/vnd.oai.openapi+json;version=3.0.4', 'application/vnd.oai.openapi+yaml;version=3.0.4');
var _default = exports.default = mediaTypes;