"use strict";

exports.__esModule = true;
exports.default = exports.CallbackVisitor = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _visitor = require("./visitor.cjs");
var _index = require("../predicates/index.cjs");
/**
 * @public
 */

/**
 * @public
 */

class CallbackVisitor extends _visitor.PredicateVisitor {
  callback;
  constructor({
    callback = _ramdaAdjunct.noop,
    ...rest
  } = {}) {
    super({
      ...rest
    });
    this.callback = callback;
  }
  enter(element) {
    if (this.predicate(element)) {
      this.callback(element);
      return this.returnOnTrue;
    }
    return this.returnOnFalse;
  }
}

/**
 * Executes the callback on this element and all descendants.
 * @public
 */
exports.CallbackVisitor = CallbackVisitor;
const traverse = (options, element) => {
  let callback;
  let predicate;
  if ((0, _ramdaAdjunct.isFunction)(options)) {
    callback = options;
    predicate = _index.isElement;
  } else {
    callback = (0, _ramda.pathOr)(_ramdaAdjunct.noop, ['callback'], options);
    predicate = (0, _ramda.pathOr)(_index.isElement, ['predicate'], options);
  }
  const visitor = new CallbackVisitor({
    callback,
    predicate
  });

  // @ts-ignore
  (0, _visitor.visit)(element, visitor);
};
var _default = exports.default = traverse;