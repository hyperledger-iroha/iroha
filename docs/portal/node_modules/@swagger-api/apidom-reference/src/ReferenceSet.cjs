"use strict";

exports.__esModule = true;
exports.default = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
/**
 * @public
 */

/**
 * @public
 */
class ReferenceSet {
  rootRef;
  refs;
  circular;
  constructor({
    refs = [],
    circular = false
  } = {}) {
    this.refs = [];
    this.circular = circular;
    refs.forEach(this.add.bind(this));
  }
  get size() {
    return this.refs.length;
  }
  add(reference) {
    if (!this.has(reference)) {
      this.refs.push(reference);
      this.rootRef = this.rootRef === undefined ? reference : this.rootRef;
      reference.refSet = this; // eslint-disable-line no-param-reassign
    }
    return this;
  }
  merge(anotherRefSet) {
    for (const reference of anotherRefSet.values()) {
      this.add(reference);
    }
    return this;
  }
  has(thing) {
    const uri = (0, _ramdaAdjunct.isString)(thing) ? thing : thing.uri;
    return (0, _ramdaAdjunct.isNotUndefined)(this.find(ref => ref.uri === uri));
  }
  find(predicate) {
    return this.refs.find(predicate);
  }
  *values() {
    yield* this.refs;
  }
  clean() {
    this.refs.forEach(ref => {
      ref.refSet = undefined; // eslint-disable-line no-param-reassign
    });
    this.rootRef = undefined;
    this.refs.length = 0;
  }
}
var _default = exports.default = ReferenceSet;