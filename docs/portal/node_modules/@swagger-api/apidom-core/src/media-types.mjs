import { NotImplementedError } from '@swagger-api/apidom-error';

/**
 * @public
 */
class MediaTypes extends Array {
  unknownMediaType = 'application/octet-stream';

  // eslint-disable-next-line class-methods-use-this
  filterByFormat() {
    throw new NotImplementedError('filterByFormat method in MediaTypes class is not yet implemented.');
  }

  // eslint-disable-next-line class-methods-use-this
  findBy() {
    throw new NotImplementedError('findBy method in MediaTypes class is not yet implemented.');
  }

  // eslint-disable-next-line class-methods-use-this
  latest() {
    throw new NotImplementedError('latest method in MediaTypes class is not yet implemented.');
  }
}
export default MediaTypes;