import { visit } from "../../traversal/visitor.mjs";
import serializeValue from "./value/index.mjs";
class YamlVisitor {
  static indentChar = '  ';
  result;
  indent;
  constructor({
    directive = false,
    indent = 0
  } = {}) {
    this.result = directive ? '%YAML 1.2\n---\n' : '';
    this.indent = indent;
  }
  NumberElement(element) {
    this.result += serializeValue(element);
  }
  BooleanElement(element) {
    const value = serializeValue(element);
    this.result += value ? 'true' : 'false';
  }
  StringElement(element) {
    // for simplicity and avoiding ambiguity we always wrap strings in quotes
    this.result += JSON.stringify(serializeValue(element));
  }
  NullElement() {
    this.result += 'null';
  }
  ArrayElement(element) {
    if (element.length === 0) {
      this.result += '[]';
      return false;
    }
    element.forEach(item => {
      const visitor = new YamlVisitor({
        indent: this.indent + 1
      });
      const indent = YamlVisitor.indentChar.repeat(this.indent);
      visit(item, visitor);
      const {
        result
      } = visitor;
      this.result += result.startsWith('\n') ? `\n${indent}-${result}` : `\n${indent}- ${result}`;
    });
    return false;
  }
  ObjectElement(element) {
    if (element.length === 0) {
      this.result += '{}';
      return false;
    }
    element.forEach((value, key) => {
      const keyVisitor = new YamlVisitor({
        indent: this.indent + 1
      });
      const valueVisitor = new YamlVisitor({
        indent: this.indent + 1
      });
      const indent = YamlVisitor.indentChar.repeat(this.indent);
      visit(key, keyVisitor);
      visit(value, valueVisitor);
      const {
        result: keyResult
      } = keyVisitor;
      const {
        result: valueResult
      } = valueVisitor;
      this.result += valueResult.startsWith('\n') ? `\n${indent}${keyResult}:${valueResult}` : `\n${indent}${keyResult}: ${valueResult}`;
    });
    return false;
  }
}

/**
 * @public
 */
const serializer = (element, {
  directive = false
} = {}) => {
  const visitor = new YamlVisitor({
    directive
  });
  visit(element, visitor);
  return visitor.result;
};
export default serializer;