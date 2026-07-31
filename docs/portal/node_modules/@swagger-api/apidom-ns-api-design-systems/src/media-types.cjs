"use strict";

exports.__esModule = true;
exports.default = exports.ApiDesignSystemsMediaTypes = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

/**
 * @public
 */
class ApiDesignSystemsMediaTypes extends _apidomCore.MediaTypes {
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
    return (0, _ramda.last)(this.filterByFormat(format));
  }
}

/**
 * @public
 */
exports.ApiDesignSystemsMediaTypes = ApiDesignSystemsMediaTypes;
const mediaTypes = new ApiDesignSystemsMediaTypes('application/vnd.aai.apidesignsystems;version=2021-05-07', 'application/vnd.aai.apidesignsystems+json;version=2021-05-07', 'application/vnd.aai.apidesignsystems+yaml;version=2021-05-07');
var _default = exports.default = mediaTypes;