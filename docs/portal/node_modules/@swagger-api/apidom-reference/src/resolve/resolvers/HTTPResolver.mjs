import Resolver from "./Resolver.mjs";
import * as url from "../../util/url.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HTTPResolver extends Resolver {
  timeout;
  redirects;
  withCredentials;
  constructor(options) {
    const {
      name = 'http-resolver',
      timeout = 5000,
      redirects = 5,
      withCredentials = false
    } = options !== null && options !== void 0 ? options : {};
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
export default HTTPResolver;