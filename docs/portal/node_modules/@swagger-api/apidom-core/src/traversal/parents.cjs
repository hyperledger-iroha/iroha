"use strict";

exports.__esModule = true;
exports.default = void 0;
var _visitor = require("./visitor.cjs");
class Visitor {
  parentEdges;
  parent;
  constructor() {
    this.parentEdges = new WeakMap();
  }
  ObjectElement(objectElement) {
    this.parentEdges.set(objectElement, this.parent);
    this.parent = objectElement;
  }
  ArrayElement(arrayElement) {
    this.parentEdges.set(arrayElement, this.parent);
    this.parent = arrayElement;
  }
  MemberElement(memberElement) {
    this.parentEdges.set(memberElement, this.parent);
    this.parent = memberElement;
  }
  enter(element) {
    this.parentEdges.set(element, this.parent);
  }
}

/**
 * Computes upwards edges from every child to its parent.
 * @public
 */
const parents = element => {
  const visitor = new Visitor();
  (0, _visitor.visit)(element, visitor);
  return visitor.parentEdges;
};
var _default = exports.default = parents;