import { curry, map } from 'ramda';

// This implementation was highly inspired by the implementations
// in ramda-lens library.
//
// https://github.com/ramda/ramda-lens

// isomorphic :: ((a -> b), (b -> a)) -> Isomorphism
//     Isomorphism = x -> y
var isomorphic = function isomorphic(to, from) {
  var isomorphism = function isomorphism(x) {
    return to(x);
  };
  isomorphism.from = from;
  return isomorphism;
};

// isomorphisms :: ((a -> b), (b -> a)) -> (a -> b)
var isomorphisms = function isomorphisms(to, from) {
  return isomorphic(curry(function (toFunctorFn, target) {
    return map(from, toFunctorFn(to(target)));
  }), curry(function (toFunctorFn, target) {
    return map(to, toFunctorFn(from(target)));
  }));
};

// from :: Isomorphism -> a -> b
var from = curry(function (isomorphism, x) {
  return isomorphism.from(x);
});

/**
 * Defines an isomorphism that will work like a lens. It takes two functions.
 * The function that converts and the function that recovers.
 *
 * @func lensIso
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/1.19.0|1.19.0}
 * @category Relation
 * @typedef Lens s a = Functor f => (a -> f a) -> s -> f s
 * @sig (s -> a) -> (a -> s) -> Lens s a
 * @param {!function} to The function that converts
 * @param {!function} from The function that recovers
 * @return {!function} The isomorphic lens
 * @see {@link http://ramdajs.com/docs/#lens|R.lens}
 *
 * @example
 *
 * const lensJSON = RA.lensIso(JSON.parse, JSON.stringify);
 *
 * R.over(lensJSON, assoc('b', 2), '{"a":1}'); //=> '{"a":1,"b":2}'
 * R.over(RA.lensIso.from(lensJSON), R.replace('}', ',"b":2}'), { a: 1 }); // => { a: 1, b: 2 }
 */
var lensIso = curry(isomorphisms);
lensIso.from = from;
export default lensIso;