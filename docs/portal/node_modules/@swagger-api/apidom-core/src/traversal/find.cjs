"use strict";

exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _visitor = require("./visitor.cjs");
/**
 * Find first element that satisfies the provided predicate.
 * @public
 */
const find = (predicate, element) => {
  const visitor = new _visitor.PredicateVisitor({
    predicate,
    returnOnTrue: _visitor.BREAK
  });
  (0, _visitor.visit)(element, visitor);
  return (0, _ramda.pathOr)(undefined, [0], visitor.result);
};
var _default = exports.default = find;