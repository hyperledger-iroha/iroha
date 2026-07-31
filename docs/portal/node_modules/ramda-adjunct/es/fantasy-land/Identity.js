function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _classCallCheck(a, n) { if (!(a instanceof n)) throw new TypeError("Cannot call a class as a function"); }
function _defineProperties(e, r) { for (var t = 0; t < r.length; t++) { var o = r[t]; o.enumerable = o.enumerable || !1, o.configurable = !0, "value" in o && (o.writable = !0), Object.defineProperty(e, _toPropertyKey(o.key), o); } }
function _createClass(e, r, t) { return r && _defineProperties(e.prototype, r), t && _defineProperties(e, t), Object.defineProperty(e, "prototype", { writable: !1 }), e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
import { empty as emptyR } from 'ramda';
import * as fl from './mapping.js';
import { applyTrait, functorTrait, setoidTrait, semigroupTrait, chainTrait, ordTrait } from './traits.js';

/**
 * The simplest {@link https://github.com/fantasyland/fantasy-land|fantasy-land}
 * compatible monad which attaches no information to values.
 *
 * The Identity type is a very simple type that has no interesting side effects and
 * is effectively just a container of some value. So why does it exist ?
 * The Identity type is often used as the base monad of a monad
 * transformer when no other behaviour is required.
 *
 * @memberOf RA
 * @implements
 * {@link https://github.com/fantasyland/fantasy-land#apply|Apply},
 * {@link https://github.com/fantasyland/fantasy-land#applicative|Applicative},
 * {@link https://github.com/fantasyland/fantasy-land#functor|Functor},
 * {@link https://github.com/fantasyland/fantasy-land#setoid|Setoid},
 * {@link https://github.com/fantasyland/fantasy-land#semigroup|Semigroup},
 * {@link https://github.com/fantasyland/fantasy-land#chain|Chain},
 * {@link https://github.com/fantasyland/fantasy-land#monad|Monad},
 * {@link https://github.com/fantasyland/fantasy-land#ord|Ord},
 * {@link https://github.com/fantasyland/fantasy-land#monoid|Monoid*},
 * {@link https://github.com/fantasyland/fantasy-land#contravariant|Contravariant}
 * @since {@link https://char0n.github.io/ramda-adjunct/1.8.0|v1.8.0}
 */
var Identity = /*#__PURE__*/function (_fl$of, _fl$ap, _fl$map, _fl$equals, _fl$concat, _fl$chain, _fl$lte, _fl$empty, _fl$contramap) {
  /**
   * Private constructor. Use {@link RA.Identity.of|Identity.of} instead.
   *
   * @param {*} value
   * @return {RA.Identity}
   */
  function Identity(value) {
    _classCallCheck(this, Identity);
    this.value = value;
  }

  /**
   * Catamorphism for a value.
   * @returns {*}
   * @example
   *
   * const a = Identity.of(1);
   * a.get(); //=> 1
   */
  return _createClass(Identity, [{
    key: "get",
    value: function get() {
      return this.value;
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#apply|Apply} specification.
     *
     * @sig ap :: Apply f => f a ~> f (a -> b) -> f b
     * @param {RA.Identity} applyWithFn
     * @return {RA.Identity}
     * @example
     *
     * const a = Identity.of(1);
     * const b = Identity.of(1).map(a => b => a + b);
     *
     * a.ap(b); //=> Identity(2)
     */
  }, {
    key: _fl$ap,
    value: function value(applyWithFn) {
      return applyTrait[fl.ap].call(this, applyWithFn);
    }
  }, {
    key: "ap",
    value: function ap(applyWithFn) {
      return this[fl.ap](applyWithFn);
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#functor|Functor} specification.
     *
     * @sig map :: Functor f => f a ~> (a -> b) -> f b
     * @param {Function} fn
     * @return {RA.Identity}
     * @example
     *
     * const a = Identity.of(1);
     * a.map(a => a + 1); //=> Identity(2)
     */
  }, {
    key: _fl$map,
    value: function value(fn) {
      return functorTrait[fl.map].call(this, fn);
    }
  }, {
    key: "map",
    value: function map(fn) {
      return this[fl.map](fn);
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#setoid|Setoid} specification.
     *
     * @sig equals :: Setoid a => a ~> a -> Boolean
     * @param {RA.Identity} setoid
     * @return {boolean}
     * @example
     *
     * const a = Identity.of(1);
     * const b = Identity.of(1);
     * const c = Identity.of(2);
     *
     * a.equals(b); //=> true
     * a.equals(c); //=> false
     */
  }, {
    key: _fl$equals,
    value: function value(setoid) {
      return setoidTrait[fl.equals].call(this, setoid);
    }
  }, {
    key: "equals",
    value: function equals(setoid) {
      return this[fl.equals](setoid);
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#semigroup|Semigroup} specification.
     *
     * @sig concat :: Semigroup a => a ~> a -> a
     * @param {RA.Identity} semigroup
     * @return {RA.Identity}
     * @example
     *
     * const a = Identity.of(1);
     * const b = Identity.of(1);
     * a.concat(b); //=> 2
     *
     * const c = Identity.of('c');
     * const d = Identity.of('d');
     * c.concat(d); //=> 'cd'
     *
     * const e = Identity.of(['e']);
     * const f = Identity.of(['f']);
     * e.concat(f); //=> ['e', 'f']
     */
  }, {
    key: _fl$concat,
    value: function value(semigroup) {
      return semigroupTrait[fl.concat].call(this, semigroup);
    }
  }, {
    key: "concat",
    value: function concat(semigroup) {
      return this[fl.concat](semigroup);
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#chain|Chain} specification.
     *
     * @sig chain :: Chain m => m a ~> (a -> m b) -> m b
     * @param {Function} fn Function returning the value of the same {@link https://github.com/fantasyland/fantasy-land#semigroup|Chain}
     * @return {RA.Identity}
     * @example
     *
     * const a = Identity.of(1);
     * const fn = val => Identity.of(val + 1);
     *
     * a.chain(fn).chain(fn); //=> Identity(3)
     */
  }, {
    key: _fl$chain,
    value: function value(fn) {
      return chainTrait[fl.chain].call(this, fn);
    }
  }, {
    key: "chain",
    value: function chain(fn) {
      return this[fl.chain](fn);
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#ord|Ord} specification.
     *
     * @sig lte :: Ord a => a ~> a -> Boolean
     * @param {RA.Identity} ord
     * @return {boolean}
     * @example
     *
     * const a = Identity.of(1);
     * const b = Identity.of(1);
     * const c = Identity.of(2);
     *
     * a.lte(b); //=> true
     * a.lte(c); //=> true
     * c.lte(a); //=> false
     */
  }, {
    key: _fl$lte,
    value: function value(ord) {
      return ordTrait[fl.lte].call(this, ord);
    }
  }, {
    key: "lte",
    value: function lte(ord) {
      return this[fl.lte](ord);
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#monoid|Monoid*} specification.
     * Partial implementation of Monoid specification. `empty` method on instance only, returning
     * identity value of the wrapped type. Using `R.empty` under the hood.
     *
     *
     * @sig empty :: Monoid m => () -> m
     * @return {RA.Identity}
     * @example
     *
     * const a = Identity.of('test');
     * const i = a.empty();
     *
     * a.concat(i); //=> Identity('string');
     * i.concat(a); //=> Identity('string');
     */
  }, {
    key: _fl$empty,
    value: function value() {
      return this.constructor.of(emptyR(this.value));
    }
  }, {
    key: "empty",
    value: function empty() {
      return this[fl.empty]();
    }

    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#contravariant|Contravariant} specification.
     *
     * @sig contramap :: Contravariant f => f a ~> (b -> a) -> f b
     * @param {Function} fn
     * @return {RA.Identity}
     * @example
     *
     * const identity = a => a;
     * const add1 = a => a + 1;
     * const divide2 = a => a / 2;
     *
     * Identity.of(divide2).contramap(add1).get()(3); //=> 2
     * Identity.of(identity).contramap(divide2).contramap(add1).get()(3); //=> 2
     * Identity.of(identity).contramap(a => divide2(add1(a))).get()(3); //=> 2
     */
  }, {
    key: _fl$contramap,
    value: function value(fn) {
      var _this = this;
      return this.constructor.of(function (value) {
        return _this.value(fn(value));
      });
    }
  }, {
    key: "contramap",
    value: function contramap(fn) {
      return this[fl.contramap](fn);
    }
  }], [{
    key: _fl$of,
    value:
    /**
     * Fantasy land {@link https://github.com/fantasyland/fantasy-land#applicative|Applicative} specification.
     *
     * @static
     * @sig of :: Applicative f => a -> f a
     * @param {*} value
     * @returns {RA.Identity}
     * @example
     *
     * const a = Identity.of(1); //=> Identity(1)
     */
    function value(_value) {
      return new Identity(_value);
    }
  }, {
    key: "of",
    value: function of(value) {
      return new Identity(value);
    }

    /**
     * @static
     */
  }, {
    key: '@@type',
    get: function get() {
      return 'RA/Identity';
    }
  }]);
}(fl.of, fl.ap, fl.map, fl.equals, fl.concat, fl.chain, fl.lte, fl.empty, fl.contramap);
export default Identity;