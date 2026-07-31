"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _visitor = require("../../traversal/visitor.cjs");
var _index = _interopRequireDefault(require("./value/index.cjs"));
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
    this.result += (0, _index.default)(element);
  }
  BooleanElement(element) {
    const value = (0, _index.default)(element);
    this.result += value ? 'true' : 'false';
  }
  StringElement(element) {
    // for simplicity and avoiding ambiguity we always wrap strings in quotes
    this.result += JSON.stringify((0, _index.default)(element));
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
      (0, _visitor.visit)(item, visitor);
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
      (0, _visitor.visit)(key, keyVisitor);
      (0, _visitor.visit)(value, valueVisitor);
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
  (0, _visitor.visit)(element, visitor);
  return visitor.result;
};
var _default = exports.default = serializer;