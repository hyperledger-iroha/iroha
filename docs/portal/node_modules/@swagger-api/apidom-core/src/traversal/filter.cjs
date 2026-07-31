"use strict";

exports.__esModule = true;
exports.default = void 0;
var _minim = require("minim");
var _visitor = require("./visitor.cjs");
/**
 * Finds all elements matching the predicate.
 * @public
 */
const filter = (predicate, element) => {
  const visitor = new _visitor.PredicateVisitor({
    predicate
  });
  (0, _visitor.visit)(element, visitor);
  return new _minim.ArraySlice(visitor.result);
};
var _default = exports.default = filter;