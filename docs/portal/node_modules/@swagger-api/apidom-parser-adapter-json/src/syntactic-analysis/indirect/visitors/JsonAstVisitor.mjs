import { JsonArray, JsonDocument, JsonObject, JsonProperty, ParseResult, Error, isNode as isCSTNode, getNodeType as getCSTNodeType } from '@swagger-api/apidom-ast';
import { ParseResultElement, ObjectElement, MemberElement, ArrayElement, BooleanElement, NullElement, NumberElement, StringElement, AnnotationElement, isParseResultElement, isPrimitiveElement, isElement, keyMap as keyMapApiDOM, getNodeType as getNodeTypeApiDOM, assignSourceMap } from '@swagger-api/apidom-core';
export const keyMap = {
  // @ts-ignore
  [ParseResult.type]: ['children'],
  // @ts-ignore
  [JsonDocument.type]: ['children'],
  // @ts-ignore
  [JsonObject.type]: ['children'],
  // @ts-ignore
  [JsonProperty.type]: ['children'],
  // @ts-ignore
  [JsonArray.type]: ['children'],
  // @ts-ignore
  [Error.type]: ['children'],
  ...keyMapApiDOM
};
export const getNodeType = node => {
  if (isParseResultElement(node)) {
    return 'ParseResultElement';
  }
  if (isElement(node)) {
    return getNodeTypeApiDOM(node);
  }
  return getCSTNodeType(node);
};
export const isNode = element => isElement(element) || isCSTNode(element);

/* eslint-disable no-underscore-dangle */

class JsonAstVisitor {
  sourceMap = false;
  annotations;
  ParseResultElement = {
    leave: element => {
      // mark first-non Annotation element as result
      // @ts-ignore
      const elements = element.findElements(isPrimitiveElement);
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
    const element = new ParseResultElement();
    // @ts-ignore
    element._content = node.children;
    return element;
  }
  object(node) {
    const element = new ObjectElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  property(node) {
    const element = new MemberElement();

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
    const element = new StringElement(node.value);
    this.maybeAddSourceMap(node, element);
    return element;
  }
  array(node) {
    const element = new ArrayElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  string(node) {
    const element = new StringElement(node.value);
    this.maybeAddSourceMap(node, element);
    return element;
  }
  number(node) {
    const element = new NumberElement(Number(node.value));
    this.maybeAddSourceMap(node, element);
    return element;
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  null(node) {
    const element = new NullElement();
    this.maybeAddSourceMap(node, element);
    return element;
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  true(node) {
    const element = new BooleanElement(true);
    this.maybeAddSourceMap(node, element);
    return element;
  }

  // eslint-disable-next-line @typescript-eslint/naming-convention
  false(node) {
    const element = new BooleanElement(false);
    this.maybeAddSourceMap(node, element);
    return element;
  }
  literal(node) {
    if (node.isMissing) {
      const message = `(Missing ${node.value})`;
      const element = new AnnotationElement(message);
      element.classes.push('warning');
      this.maybeAddSourceMap(node, element);
      this.annotations.push(element);
    }
    return null;
  }
  error(node, key, parent, path) {
    const message = node.isUnexpected ? `(Unexpected ${node.value})` : `(Error ${node.value})`;
    const element = new AnnotationElement(message);
    element.classes.push('error');
    this.maybeAddSourceMap(node, element);
    if (path.length === 0) {
      // no document to visit, only error is present in CST
      const parseResultElement = new ParseResultElement();
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
    assignSourceMap(element, node);
  }
}
export default JsonAstVisitor;