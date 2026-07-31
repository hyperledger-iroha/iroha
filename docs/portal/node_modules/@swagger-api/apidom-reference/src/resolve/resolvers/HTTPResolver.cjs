"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Resolver = _interopRequireDefault(require("./Resolver.cjs"));
var url = _interopRequireWildcard(require("../../util/url.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class HTTPResolver extends _Resolver.default {
  timeout;
  redirects;
  withCredentials;
  constructor(options) {
    const {
      name = 'http-resolver',
      timeout = 5000,
      redirects = 5,
      withCredentials = false
    } = options != null ? options : {};
    super({
      name
    });
    this.timeout = timeout;
    this.redirects = redirects;
    this.withCredentials = withCredentials;
  }

  // eslint-disable-next-line class-methods-use-this
  canRead(file) {
    return url.isHttpUrl(file.uri);
  }
}
var _default = exports.default = HTTPResolver;