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
  stream() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  yaml_directive() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  tag_directive() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  reserved_directive() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  document() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  block_node() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  flow_node() {
    return new _TreeCursorSyntaxNode.default(this.cursor).setFieldName(this.cursor);
  }
  block_mapping() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  block_mapping_pair() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  flow_mapping() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  flow_pair() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  block_sequence() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  block_sequence_item() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  flow_sequence() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  plain_scalar() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  single_quote_scalar() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  double_quote_scalar() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
  }
  block_scalar() {
    return new _TreeCursorSyntaxNode.default(this.cursor);
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
        const firstChildSiblings = Array.from(new TreeCursorIterator(this.cursor));
        node.pushChildren(...firstChildSiblings);
      }
      node.children.reduce((previousNode, currentNode) => {
        currentNode.setPreviousSibling(previousNode);
        return currentNode;
      }, undefined);
      this.cursor.gotoParent();
    }
    yield node;
  }
}
var _default = exports.default = TreeCursorIterator;