import TreeCursorSyntaxNode from "./TreeCursorSyntaxNode.mjs";
class TreeCursorIterator {
  cursor;
  constructor(cursor) {
    this.cursor = cursor;
  }
  stream() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  yaml_directive() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  tag_directive() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  reserved_directive() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  document() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  block_node() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  flow_node() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  block_mapping() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  block_mapping_pair() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  flow_mapping() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  flow_pair() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  block_sequence() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  block_sequence_item() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  flow_sequence() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  plain_scalar() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  single_quote_scalar() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  double_quote_scalar() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  block_scalar() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  ERROR() {
    return new TreeCursorSyntaxNode(this.cursor).setHasError(this.cursor);
  }
  *[Symbol.iterator]() {
    let node;
    if (this.cursor.nodeType in this) {
      // @ts-ignore
      node = this[this.cursor.nodeType]();
    } else {
      node = new TreeCursorSyntaxNode(this.cursor);
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
export default TreeCursorIterator;