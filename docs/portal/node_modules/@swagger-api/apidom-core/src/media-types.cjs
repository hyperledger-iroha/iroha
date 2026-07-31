"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
/**
 * @public
 */
class MediaTypes extends Array {
  unknownMediaType = 'application/octet-stream';

  // eslint-disable-next-line class-methods-use-this
  filterByFormat() {
    throw new _apidomError.NotImplementedError('filterByFormat method in MediaTypes class is not yet implemented.');
  }

  // eslint-disable-next-line class-methods-use-this
  findBy() {
    throw new _apidomError.NotImplementedError('findBy method in MediaTypes class is not yet implemented.');
  }

  // eslint-disable-next-line class-methods-use-this
  latest() {
    throw new _apidomError.NotImplementedError('latest method in MediaTypes class is not yet implemented.');
  }
}
var _default = exports.default = MediaTypes;