import { JsonArray, JsonDocument, JsonFalse, JsonNull, JsonNumber, JsonObject, JsonKey, JsonProperty, JsonString, JsonStringContent, JsonTrue, ParseResult, Literal, Error } from '@swagger-api/apidom-ast';
import { assignSourceMap } from '@swagger-api/apidom-core';
import TreeCursorSyntaxNode from "../../TreeCursorSyntaxNode.mjs";
export const keyMap = {
  document: ['children'],
  object: ['children'],
  array: ['children'],
  string: ['children'],
  property: ['children'],
  key: ['children'],
  error: ['children']
};

/* eslint-disable class-methods-use-this */

class CstVisitor {
  document = {
    enter: node => {
      return new JsonDocument(assignSourceMap({
        children: node.children,
        isMissing: node.isMissing
      }, node));
    },
    leave: document => {
      return new ParseResult({
        children: [document]
      });
    }
  };
  enter(node) {
    // anonymous literals from CST transformed into AST literal nodes
    if (node instanceof TreeCursorSyntaxNode && !node.isNamed) {
      const value = node.type || node.text;
      const {
        isMissing
      } = node;
      return new Literal(assignSourceMap({
        value,
        isMissing
      }, node));
    }
    return undefined;
  }
  object(node) {
    return new JsonObject(assignSourceMap({
      children: node.children,
      isMissing: node.isMissing
    }, node));
  }
  pair(node) {
    const children = node.children.slice(1);
    const {
      keyNode
    } = node;
    const key = new JsonKey(assignSourceMap({
      children: (keyNode === null || keyNode === void 0 ? void 0 : keyNode.children) || [],
      isMissing: keyNode != null ? keyNode.isMissing : false
    }, keyNode));
    return new JsonProperty(assignSourceMap({
      children: [key, ...children],
      isMissing: node.isMissing
    }, node));
  }
  array(node) {
    return new JsonArray(assignSourceMap({
      children: node.children,
      isMissing: node.isMissing
    }, node));
  }
  string(node) {
    const content = new JsonStringContent({
      value: JSON.parse(node.text)
    });
    return new JsonString(assignSourceMap({
      children: [content],
      isMissing: node.isMissing
    }, node));
  }
  number(node) {
    const value = node.text;
    return new JsonNumber(assignSourceMap({
      value,
      isMissing: node.isMissing
    }, node));
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  null(node) {
    const value = node.text;
    return new JsonNull(assignSourceMap({
      value,
      isMissing: node.isMissing
    }, node));
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  true(node) {
    const value = node.text;
    return new JsonTrue(assignSourceMap({
      value,
      isMissing: node.isMissing
    }, node));
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  false(node) {
    const value = node.text;
    return new JsonFalse(assignSourceMap({
      value,
      isMissing: node.isMissing
    }, node));
  }
  ERROR(node, key, parent, path) {
    const errorNode = new Error(assignSourceMap({
      children: node.children,
      isUnexpected: !node.hasError,
      isMissing: node.isMissing,
      value: node.text
    }, node));
    if (path.length === 0) {
      return new ParseResult({
        children: [errorNode]
      });
    }
    return errorNode;
  }
}
export default CstVisitor;