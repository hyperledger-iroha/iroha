import { isNotUndefined, isString } from 'ramda-adjunct';

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
    const uri = isString(thing) ? thing : thing.uri;
    return isNotUndefined(this.find(ref => ref.uri === uri));
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
export default ReferenceSet;