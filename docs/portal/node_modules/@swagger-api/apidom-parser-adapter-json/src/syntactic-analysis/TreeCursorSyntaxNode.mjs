class TreeCursorSyntaxNode {
  type;
  startPositionRow;
  startPositionColumn;
  startIndex;
  endPositionRow;
  endPositionColumn;
  endIndex;
  text;
  isNamed;
  isMissing;
  fieldName;
  hasError = false;
  children = [];
  constructor(cursor) {
    this.type = cursor.nodeType;
    this.startPositionRow = cursor.startPosition.row;
    this.startPositionColumn = cursor.startPosition.column;
    this.startIndex = cursor.startIndex;
    this.endPositionRow = cursor.endPosition.row;
    this.endPositionColumn = cursor.endPosition.column;
    this.endIndex = cursor.endIndex;
    this.text = cursor.nodeText;
    this.isNamed = cursor.nodeIsNamed;
    this.isMissing = cursor.nodeIsMissing;
  }
  get keyNode() {
    if (this.type === 'pair') {
      return this.children.find(node => node.fieldName === 'key');
    }
    return undefined;
  }
  get valueNode() {
    if (this.type === 'pair') {
      return this.children.find(node => node.fieldName === 'value');
    }
    return undefined;
  }
  setFieldName(cursor) {
    this.fieldName = cursor.currentFieldName;
    return this;
  }
  setHasError(cursor) {
    this.hasError = cursor.currentNode.hasError;
    return this;
  }
  pushChildren(...children) {
    this.children.push(...children);
  }
}
export default TreeCursorSyntaxNode;