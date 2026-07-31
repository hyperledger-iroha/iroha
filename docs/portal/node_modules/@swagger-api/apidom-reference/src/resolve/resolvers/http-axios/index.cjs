"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _axios = _interopRequireDefault(require("axios"));
var _HTTPResolver = _interopRequireDefault(require("../HTTPResolver.cjs"));
var _ResolverError = _interopRequireDefault(require("../../../errors/ResolverError.cjs"));
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
class HTTPResolverAxios extends _HTTPResolver.default {
  axiosConfig = {};
  axiosInstance;
  previousAxiosConfig;
  constructor(options) {
    const {
      axiosConfig = {},
      ...rest
    } = options != null ? options : {};
    super({
      ...rest,
      name: 'http-axios'
    });
    this.axiosConfig = axiosConfig;
  }
  getHttpClient() {
    if (this.axiosInstance === undefined || this.previousAxiosConfig !== this.axiosConfig) {
      const config = (0, _ramda.omit)(['interceptors'], this.axiosConfig);
      const {
        interceptors
      } = this.axiosConfig;
      this.axiosInstance = _axios.default.create({
        timeout: this.timeout,
        maxRedirects: this.redirects,
        withCredentials: this.withCredentials,
        responseType: 'arraybuffer',
        ...config
      });

      // settings up request interceptors
      if (Array.isArray(interceptors == null ? void 0 : interceptors.request)) {
        interceptors.request.forEach(requestInterceptor => {
          this.axiosInstance.interceptors.request.use(...(0, _ramdaAdjunct.ensureArray)(requestInterceptor));
        });
      }

      // settings up response interceptors
      if (Array.isArray(interceptors == null ? void 0 : interceptors.response)) {
        interceptors.response.forEach(responseInterceptor => {
          this.axiosInstance.interceptors.response.use(...(0, _ramdaAdjunct.ensureArray)(responseInterceptor));
        });
      }
      this.previousAxiosConfig = this.axiosConfig;
    }
    return this.axiosInstance;
  }
  async read(file) {
    const client = this.getHttpClient();
    try {
      const response = await client.get(file.uri);
      return response.data;
    } catch (error) {
      throw new _ResolverError.default(`Error downloading "${file.uri}"`, {
        cause: error
      });
    }
  }
}
var _default = exports.default = HTTPResolverAxios;