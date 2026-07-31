import isArrayLike from '../isArrayLike.js';

/**
 * `makeFlat` is a helper function that returns a one-level or fully recursive
 * function based on the flag passed in.
 *
 * @func makeFlat
 * @memberOf RA
 *
 * @category List
 * @param {!bool} = should recursively flatten
 * @param {!Array} = the nested list to be flattened
 * @return {!Array} = the flattened list
 * @sig Bool -> List -> List
 *
 */
var makeFlat = function makeFlat(recursive) {
  return function flatt(list) {
    var value;
    var jlen;
    var j;
    var result = [];
    var idx = 0;
    while (idx < list.length) {
      if (isArrayLike(list[idx])) {
        value = recursive ? flatt(list[idx]) : list[idx];
        j = 0;
        jlen = value.length;
        while (j < jlen) {
          result[result.length] = value[j];
          j += 1;
        }
      } else {
        result[result.length] = list[idx];
      }
      idx += 1;
    }
    return result;
  };
};
export default makeFlat;