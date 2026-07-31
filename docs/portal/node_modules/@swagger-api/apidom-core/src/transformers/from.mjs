import { has } from 'ramda';
import { isPlainObject, isString } from 'ramda-adjunct';
import defaultNamespaceInstance from "../namespace.mjs";
/**
 * Transforms data to an Element from a particular namespace.
 *
 * The name of the function was originally `from`,
 * but it was renamed to `fromFn` to avoid issues with Parcel.js:
 *
 * - https://github.com/parcel-bundler/parcel/issues/9473
 * - https://github.com/swagger-api/swagger-ui/issues/9466#issuecomment-1881053410
 * @public
 */
const fromFn = (data, namespace = defaultNamespaceInstance) => {
  if (isString(data)) {
    // JSON serialized refract
    try {
      return namespace.fromRefract(JSON.parse(data));
    } catch {
      // noop
    }
  }
  if (isPlainObject(data) && has('element', data)) {
    // refract javascript structure
    return namespace.fromRefract(data);
  }
  return namespace.toElement(data);
};
export default fromFn;