function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: !0, configurable: !0, writable: !0 }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
import { curry } from 'ramda';
import renameKeys from './renameKeys.js';

/**
 * Creates a new object with the own properties of the provided object, but a
 * single key is renamed from `oldKey` to `newKey`.
 *
 * Keep in mind that in the case of keys conflict is behavior undefined and
 * the result may vary between various JS engines!
 *
 * @func renameKey
 * @memberOf RA
 * @since {@link https://char0n.github.io/ramda-adjunct/4.1.0|v4.1.0}
 * @category Object
 * @sig (String a, String b) => a -> b ->  {a: *} -> {b: *}
 * @param {!string} oldKey
 * @param {!string} newKey
 * @param {!Object} obj
 * @return {!Object} New object with renamed key
 * @see {@link https://github.com/ramda/ramda/wiki/Cookbook#rename-key-of-an-object|Ramda Cookbook}, {@link RA.renameKeyWith|renameKeyWith}
 * @example
 *
 * const input = { firstName: 'Elisia', age: 22, type: 'human' };
 *
 * RA.renameKey('firstName', 'name')(input);
 * //=> { name: 'Elisia', age: 22, type: 'human' }
 */
var renameKey = curry(function (oldKey, newKey, obj) {
  return renameKeys(_defineProperty({}, oldKey, newKey), obj);
});
export default renameKey;