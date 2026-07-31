"use strict";

exports.__esModule = true;
exports.keyMap = exports.isNode = exports.getNodeType = exports.default = void 0;
var _apidomAst = require("@swagger-api/apidom-ast");
var _apidomCore = require("@swagger-api/apidom-core");
const keyMap = exports.keyMap = {
  // @ts-ignore
  [_apidomAst.ParseResult.type]: ['children'],
  // @ts-ignore
  [_apidomAst.JsonDocument.type]: ['children'],
  // @ts-ignore
  [_apidomAst.JsonObject.type]: ['children'],
  // @ts-ignore
  [_apidomAst.JsonProperty.type]: ['children'],
  // @ts-ignore
  [_apidomAst.JsonArray.type]: ['children'],
  // @ts-ignore
  [_apidomAst.Error.type]: ['children'],
  ..._apidomCore.keyMap
};
const getNodeType = node => {
  if ((0, _apidomCore.isParseResultElement)(node)) {
    return 'ParseResultElement';
  }
  if ((0, _apidomCore.isElement)(node)) {
    return (0, _apidomCore.getNodeType)(node);
  }
  return (0, _apidomAst.getNodeType)(node);
};
exports.getNodeType = getNodeType;
const isNode = element => (0, _apidomCore.isElement)(element) || (0, _apidomAst.isNode)(element);

/* eslint-disable no-underscore-dangle */
exports.isNode = isNode;
class JsonAstVisitor {
  sourceMap = false;
  annotations;
  ParseResultElement = {
    leave: element => {
      // mark first-non Annotation element as result
      // @ts-ignore
      const elements = element.findElements(_apidomCore.isPrimitiveElement);
      if (elements.length > 0) {
        const resultElement = elements[0];
        resultElement.classes.push('result');
      }

      // provide annotations
      this.annotations.forEach(annotationElement => {
        element.push(annotationElement);
      });
      this.annotations = [];
    }
  };
  constructor() {
    this.annotations = [];
  }

  // eslint-disable-next-line class-methods-use-this
  document(node) {
    const element = new _apidomCore.ParseResultElement();
    // @ts-ignore
    element._content = node.children;
    return element;
  }
  object(node) {
    const element = new _apidomCore.ObjectElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  property(node) {
    const element = new _apidomCore.MemberElement();

    // @ts-ignore
    element.content.key = node.key;
    // @ts-ignore
    element.content.value = node.value;
    this.maybeAddSourceMap(node, element);

    /**
     * Process possible errors here that may be present in pair node children as we're using direct field access.
     * There are usually 3 children here found: "key", ":", "value".
     */
    if (node.children.length > 3) {
      node.children
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      .filter(child => child.type === 'error')
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      .forEach(errorNode => {
        this.error(errorNode, node, [], [node]);
      });
    }
    return element;
  }
  key(node) {
    const element = new _apidomCore.StringElement(node.value);
    this.maybeAddSourceMap(node, element);
    return element;
  }
  array(node) {
    const element = new _apidomCore.ArrayElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  string(node) {
    const element = new _apidomCore.StringElement(node.value);
    this.maybeAddSourceMap(node, element);
    return element;
  }
  number(node) {
    const element = new _apidomCore.NumberElement(Number(node.value));
    this.maybeAddSourceMap(node, element);
    return element;
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  null(node) {
    const element = new _apidomCore.NullElement();
    this.maybeAddSourceMap(node, element);
    return element;
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  true(node) {
    const element = new _apidomCore.BooleanElement(true);
    this.maybeAddSourceMap(node, element);
    return element;
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  false(node) {
    const element = new _apidomCore.BooleanElement(false);
    this.maybeAddSourceMap(node, element);
    return element;
  }
  literal(node) {
    if (node.isMissing) {
      const message = `(Missing ${node.value})`;
      const element = new _apidomCore.AnnotationElement(message);
      element.classes.push('warning');
      this.maybeAddSourceMap(node, element);
      this.annotations.push(element);
    }
    return null;
  }
  error(node, key, parent, path) {
    const message = node.isUnexpected ? `(Unexpected ${node.value})` : `(Error ${node.value})`;
    const element = new _apidomCore.AnnotationElement(message);
    element.classes.push('error');
    this.maybeAddSourceMap(node, element);
    if (path.length === 0) {
      // no document to visit, only error is present in CST
      const parseResultElement = new _apidomCore.ParseResultElement();
      parseResultElement.push(element);
      return parseResultElement;
    }
    this.annotations.push(element);
    return null;
  }
  maybeAddSourceMap(node, element) {
    if (!this.sourceMap) {
      return;
    }
    (0, _apidomCore.assignSourceMap)(element, node);
  }
}
var _default = exports.default = JsonAstVisitor;