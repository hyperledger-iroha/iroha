"use strict";

exports.__esModule = true;
exports.default = void 0;
var _visitor = require("../traversal/visitor.cjs");
class SymbolicExpressionsVisitor {
  result = '';
  nestingLevel = 0;
  enter(element) {
    const {
      element: elementName
    } = element;
    const capitalizedElementName = elementName.charAt(0).toUpperCase() + elementName.slice(1);
    const indent = '  '.repeat(this.nestingLevel);
    this.result += this.nestingLevel > 0 ? '\n' : '';
    this.result += `${indent}(${capitalizedElementName}Element`;
    this.nestingLevel += 1;
  }
  leave() {
    this.nestingLevel -= 1;
    this.result += ')';
  }
}

/**
 * Transforms ApiDOM into S-expressions (Symbolic Expressions).
 * @public
 */
const sexprs = element => {
  const visitor = new SymbolicExpressionsVisitor();
  (0, _visitor.visit)(element, visitor);
  return visitor.result;
};
var _default = exports.default = sexprs;