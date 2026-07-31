"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.keyMap = exports.default = void 0;
var _apidomAst = require("@swagger-api/apidom-ast");
var _apidomCore = require("@swagger-api/apidom-core");
var _TreeCursorSyntaxNode = _interopRequireDefault(require("../../TreeCursorSyntaxNode.cjs"));
const keyMap = exports.keyMap = {
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
      return new _apidomAst.JsonDocument((0, _apidomCore.assignSourceMap)({
        children: node.children,
        isMissing: node.isMissing
      }, node));
    },
    leave: document => {
      return new _apidomAst.ParseResult({
        children: [document]
      });
    }
  };
  enter(node) {
    // anonymous literals from CST transformed into AST literal nodes
    if (node instanceof _TreeCursorSyntaxNode.default && !node.isNamed) {
      const value = node.type || node.text;
      const {
        isMissing
      } = node;
      return new _apidomAst.Literal((0, _apidomCore.assignSourceMap)({
        value,
        isMissing
      }, node));
    }
    return undefined;
  }
  object(node) {
    return new _apidomAst.JsonObject((0, _apidomCore.assignSourceMap)({
      children: node.children,
      isMissing: node.isMissing
    }, node));
  }
  pair(node) {
    const children = node.children.slice(1);
    const {
      keyNode
    } = node;
    const key = new _apidomAst.JsonKey((0, _apidomCore.assignSourceMap)({
      children: (keyNode == null ? void 0 : keyNode.children) || [],
      isMissing: keyNode != null ? keyNode.isMissing : false
    }, keyNode));
    return new _apidomAst.JsonProperty((0, _apidomCore.assignSourceMap)({
      children: [key, ...children],
      isMissing: node.isMissing
    }, node));
  }
  array(node) {
    return new _apidomAst.JsonArray((0, _apidomCore.assignSourceMap)({
      children: node.children,
      isMissing: node.isMissing
    }, node));
  }
  string(node) {
    const content = new _apidomAst.JsonStringContent({
      value: JSON.parse(node.text)
    });
    return new _apidomAst.JsonString((0, _apidomCore.assignSourceMap)({
      children: [content],
      isMissing: node.isMissing
    }, node));
  }
  number(node) {
    const value = node.text;
    return new _apidomAst.JsonNumber((0, _apidomCore.assignSourceMap)({
      value,
      isMissing: node.isMissing
    }, node));
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  null(node) {
    const value = node.text;
    return new _apidomAst.JsonNull((0, _apidomCore.assignSourceMap)({
      value,
      isMissing: node.isMissing
    }, node));
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  true(node) {
    const value = node.text;
    return new _apidomAst.JsonTrue((0, _apidomCore.assignSourceMap)({
      value,
      isMissing: node.isMissing
    }, node));
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  false(node) {
    const value = node.text;
    return new _apidomAst.JsonFalse((0, _apidomCore.assignSourceMap)({
      value,
      isMissing: node.isMissing
    }, node));
  }
  ERROR(node, key, parent, path) {
    const errorNode = new _apidomAst.Error((0, _apidomCore.assignSourceMap)({
      children: node.children,
      isUnexpected: !node.hasError,
      isMissing: node.isMissing,
      value: node.text
    }, node));
    if (path.length === 0) {
      return new _apidomAst.ParseResult({
        children: [errorNode]
      });
    }
    return errorNode;
  }
}
var _default = exports.default = CstVisitor;