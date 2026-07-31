import Resolver from "../Resolver.mjs";
import ResolverError from "../../../errors/ResolverError.mjs";
/**
 * @public
 */
class FileResolver extends Resolver {
  constructor() {
    super({
      name: 'file'
    });
  }

  // eslint-disable-next-line class-methods-use-this
  canRead() {
    return false;
  }

  // eslint-disable-next-line class-methods-use-this
  read() {
    throw new ResolverError('FileResolver is not intended to be used in browser context.');
  }
}
export default FileResolver;