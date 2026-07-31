import isFunction from '../../isFunction.js';
import isNotUndefined from '../../isNotUndefined.js';
import repeat from './String.repeat.js';
var padEndPonyfill = function padEndPonyfill(padString, targetLength, value) {
  // eslint-disable-next-line no-bitwise
  var finalLength = targetLength >> 0;
  var finalPadString = String(isNotUndefined(padString) ? padString : ' ');
  if (value.length > finalLength) {
    return String(value);
  }
  finalLength -= value.length;
  if (finalLength > finalPadString.length) {
    var remainingLength = finalLength / finalPadString.length;
    finalPadString += isFunction(String.prototype.repeat) ? finalPadString.repeat(remainingLength) : repeat(finalPadString, remainingLength);
  }
  return String(value) + finalPadString.slice(0, finalLength);
};
export default padEndPonyfill;