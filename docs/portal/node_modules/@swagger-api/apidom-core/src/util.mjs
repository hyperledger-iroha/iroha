import { defaultTo, has, mapObjIndexed, path, propSatisfies } from 'ramda';
import { isPlainObject, isString, trimCharsStart } from 'ramda-adjunct';

/**
 * This dereference algorithm is used exclusively for dereferencing specification objects.
 * It doesn't handle circular references of external references and works on objects only (not arrays).
 * @public
 */
// eslint-disable-next-line import/prefer-default-export
export const dereference = (object, root) => {
  const rootObject = defaultTo(object, root);
  return mapObjIndexed(val => {
    if (isPlainObject(val) && has('$ref', val) && propSatisfies(isString, '$ref', val)) {
      const $ref = path(['$ref'], val);
      // @ts-ignore
      const pointer = trimCharsStart('#/', $ref);
      return path(pointer.split('/'), rootObject);
    }
    if (isPlainObject(val)) {
      return dereference(val, rootObject);
    }
    return val;
  }, object);
};
/**
 * @public
 */
/* eslint-disable no-param-reassign */
export const assignSourceMap = (to, from) => {
  to.startPositionRow = from === null || from === void 0 ? void 0 : from.startPositionRow;
  to.startPositionColumn = from === null || from === void 0 ? void 0 : from.startPositionColumn;
  to.startIndex = from === null || from === void 0 ? void 0 : from.startIndex;
  to.endPositionRow = from === null || from === void 0 ? void 0 : from.endPositionRow;
  to.endPositionColumn = from === null || from === void 0 ? void 0 : from.endPositionColumn;
  to.endIndex = from === null || from === void 0 ? void 0 : from.endIndex;
  return to;
};
/* eslint-enable no-param-reassign */