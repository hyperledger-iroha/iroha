"use strict";

exports.__esModule = true;
exports.default = exports.JSONSchemaDraft7MediaTypes = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
class JSONSchemaDraft7MediaTypes extends _apidomCore.MediaTypes {
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
    return (0, _ramda.last)(this.filterByFormat(format));
  }
}

/**
 * @public
 */
exports.JSONSchemaDraft7MediaTypes = JSONSchemaDraft7MediaTypes;
const mediaTypes = new JSONSchemaDraft7MediaTypes('application/schema;version=draft-07', 'application/schema+json;version=draft-07', 'application/schema+yaml;version=draft-07');
var _default = exports.default = mediaTypes;