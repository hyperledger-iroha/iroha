"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.evaluate = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _EvaluationElementIdError = _interopRequireDefault(require("../../../../errors/EvaluationElementIdError.cjs"));
exports.EvaluationElementIdError = _EvaluationElementIdError.default;
const hasElementID = element => !!element.id;

/**
 * Evaluates element ID against ApiDOM fragment.
 * @public
 */
const evaluate = (elementID, element) => {
  const {
    cache
  } = evaluate;
  // warm the cache
  if (!cache.has(element)) {
    const elementsWithID = (0, _apidomCore.filter)(hasElementID, element);
    cache.set(element, Array.from(elementsWithID));
  }

  // search for the matching element
  const result = cache.get(element).find(e => {
    return String((0, _apidomCore.toValue)(e.id)) === elementID;
  });
  if ((0, _ramdaAdjunct.isUndefined)(result)) {
    throw new _EvaluationElementIdError.default(`Evaluation failed on element ID: "${elementID}"`);
  }
  return result;
};
exports.evaluate = evaluate;
evaluate.cache = new WeakMap();