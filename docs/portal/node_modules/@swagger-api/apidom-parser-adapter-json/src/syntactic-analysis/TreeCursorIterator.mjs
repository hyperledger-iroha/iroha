import TreeCursorSyntaxNode from "./TreeCursorSyntaxNode.mjs";
class TreeCursorIterator {
  cursor;
  constructor(cursor) {
    this.cursor = cursor;
  }
  document() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  object() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  array() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  pair() {
    return new TreeCursorSyntaxNode(this.cursor);
  }
  string() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  number() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  null() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  true() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
  }
  false() {
    return new TreeCursorSyntaxNode(this.cursor).setFieldName(this.cursor);
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
        const firstChildSiblings = new TreeCursorIterator(this.cursor);
        node.pushChildren(...firstChildSiblings);
      }
      this.cursor.gotoParent();
    }
    yield node;
  }
}
export default TreeCursorIterator;