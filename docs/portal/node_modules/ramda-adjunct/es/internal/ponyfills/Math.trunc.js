import isFinite from '../../isFinite.js';
var truncPonyfill = function truncPonyfill(v) {
  var numV = Number(v);
  if (!isFinite(numV)) {
    return numV;
  }

  // eslint-disable-next-line no-nested-ternary
  return numV - numV % 1 || (numV < 0 ? -0 : numV === 0 ? numV : 0);
};
export default truncPonyfill;