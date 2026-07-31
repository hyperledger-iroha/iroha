"use strict";

exports.__esModule = true;
exports.dereference = exports.assignSourceMap = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
/**
 * This dereference algorithm is used exclusively for dereferencing specification objects.
 * It doesn't handle circular references of external references and works on objects only (not arrays).
 * @public
 */
// eslint-disable-next-line import/prefer-default-export
const dereference = (object, root) => {
  const rootObject = (0, _ramda.defaultTo)(object, root);
  return (0, _ramda.mapObjIndexed)(val => {
    if ((0, _ramdaAdjunct.isPlainObject)(val) && (0, _ramda.has)('$ref', val) && (0, _ramda.propSatisfies)(_ramdaAdjunct.isString, '$ref', val)) {
      const $ref = (0, _ramda.path)(['$ref'], val);
      // @ts-ignore
      const pointer = (0, _ramdaAdjunct.trimCharsStart)('#/', $ref);
      return (0, _ramda.path)(pointer.split('/'), rootObject);
    }
    if ((0, _ramdaAdjunct.isPlainObject)(val)) {
      return dereference(val, rootObject);
    }
    return val;
  }, object);
};
exports.dereference = dereference;
/**
 * @public
 */
/* eslint-disable no-param-reassign */
const assignSourceMap = (to, from) => {
  to.startPositionRow = from == null ? void 0 : from.startPositionRow;
  to.startPositionColumn = from == null ? void 0 : from.startPositionColumn;
  to.startIndex = from == null ? void 0 : from.startIndex;
  to.endPositionRow = from == null ? void 0 : from.endPositionRow;
  to.endPositionColumn = from == null ? void 0 : from.endPositionColumn;
  to.endIndex = from == null ? void 0 : from.endIndex;
  return to;
};
/* eslint-enable no-param-reassign */
exports.assignSourceMap = assignSourceMap;