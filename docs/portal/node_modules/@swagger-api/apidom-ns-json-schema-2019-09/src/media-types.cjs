"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchema201909MediaTypes = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
class JSONSchema201909MediaTypes extends _apidomCore.MediaTypes {
  filterByFormat(format = 'generic') {
    const effectiveFormat = format === 'generic' ? 'schema;version' : format;
    return this.filter(mediaType => mediaType.includes(effectiveFormat));
  }
  findBy(version = '2019-09', format = 'generic') {
    const search = format === 'generic' ? `schema;version=${version}` : `schema+${format};version=${version}`;
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
exports.JSONSchema201909MediaTypes = JSONSchema201909MediaTypes;
const mediaTypes = new JSONSchema201909MediaTypes('application/schema;version=2019-09', 'application/schema+json;version=2019-09', 'application/schema+yaml;version=2019-09');
var _default = exports.default = mediaTypes;