import { omit } from 'ramda';
import { ensureArray } from 'ramda-adjunct';
import axios from 'axios';
import HTTPResolver from "../HTTPResolver.mjs";
import ResolverError from "../../../errors/ResolverError.mjs";
/**
 * @public
 */
/**
 * @public
 */
/**
 * @public
 */
class HTTPResolverAxios extends HTTPResolver {
  axiosConfig = {};
  axiosInstance;
  previousAxiosConfig;
  constructor(options) {
    const {
      axiosConfig = {},
      ...rest
    } = options !== null && options !== void 0 ? options : {};
    super({
      ...rest,
      name: 'http-axios'
    });
    this.axiosConfig = axiosConfig;
  }
  getHttpClient() {
    if (this.axiosInstance === undefined || this.previousAxiosConfig !== this.axiosConfig) {
      const config = omit(['interceptors'], this.axiosConfig);
      const {
        interceptors
      } = this.axiosConfig;
      this.axiosInstance = axios.create({
        timeout: this.timeout,
        maxRedirects: this.redirects,
        withCredentials: this.withCredentials,
        responseType: 'arraybuffer',
        ...config
      });

      // settings up request interceptors
      if (Array.isArray(interceptors === null || interceptors === void 0 ? void 0 : interceptors.request)) {
        interceptors.request.forEach(requestInterceptor => {
          this.axiosInstance.interceptors.request.use(...ensureArray(requestInterceptor));
        });
      }

      // settings up response interceptors
      if (Array.isArray(interceptors === null || interceptors === void 0 ? void 0 : interceptors.response)) {
        interceptors.response.forEach(responseInterceptor => {
          this.axiosInstance.interceptors.response.use(...ensureArray(responseInterceptor));
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
      throw new ResolverError(`Error downloading "${file.uri}"`, {
        cause: error
      });
    }
  }
}
export default HTTPResolverAxios;