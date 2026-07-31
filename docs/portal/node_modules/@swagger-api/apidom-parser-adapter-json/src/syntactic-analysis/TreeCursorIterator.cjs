"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _TreeCursorSyntaxNode = _interopRequireDefault(require("./TreeCursorSyntaxNode.cjs"));
class TreeCursorIterator {
  cursor;
  constructor(cursor) {
    this.cursor = cursor;
  }
  document() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  object() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  array() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  pair() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  string() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  number() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  null() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  true() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  false() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  ERROR() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setHasError(this.cursor);
  }
  *[Symbol.iterator]() {
    let node;
    if (this.cursor.nodeType in this) {
      // @ts-ignore
      node = this[this.cursor.nodeType]();
    } else {
      node = new _TreeCursorSyntaxNode.default(this.cursor);
    }
    if (this.cursor.gotoFirstChild()) {
      const [firstChild] = new TreeCursorIterator(this.cursor);
      node.pushChildren(firstChild);
      while (this.cursor.gotoNextSibling()) {
        const firstChildSiblings = new TreeCursorIterator(this.cursor);
        node.pushChildren(...firstChildSiblings);
      }
      this.cursor.gotoParent();
    }
    yield node;
  }
}
var _default = exports.default = TreeCursorIterator;