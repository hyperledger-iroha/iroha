import { getNodeType as getCSTNodeType, isNode as isCSTNode, YamlStyle } from '@swagger-api/apidom-ast';
import { ParseResultElement, AnnotationElement, CommentElement, MemberElement, ObjectElement, ArrayElement, isPrimitiveElement, isElement, keyMap as keyMapApiDOM, getNodeType as getNodeTypeApiDOM, createNamespace, assignSourceMap } from '@swagger-api/apidom-core';
export const keyMap = {
  stream: ['children'],
  document: ['children'],
  mapping: ['children'],
  keyValuePair: ['children'],
  sequence: ['children'],
  error: ['children'],
  ...keyMapApiDOM
};
export const getNodeType = node => {
  if (isElement(node)) {
    return getNodeTypeApiDOM(node);
  }
  return getCSTNodeType(node);
};
export const isNode = node => isElement(node) || isCSTNode(node) || Array.isArray(node);

/* eslint-disable no-underscore-dangle */

class YamlAstVisitor {
  sourceMap = false;
  annotations;
  namespace;
  processedDocumentCount = 0;
  stream = {
    leave: node => {
      const element = new ParseResultElement();
      // @ts-ignore
      element._content = node.children.flat(1);

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
      return element;
    }
  };
  constructor() {
    this.annotations = [];
    this.namespace = createNamespace();
  }
  comment(node) {
    const isStreamComment = this.processedDocumentCount === 0;

    // we're only interested of stream comments before the first document
    if (isStreamComment) {
      // @ts-ignore
      const element = new CommentElement(node.content);
      this.maybeAddSourceMap(node, element);
      return element;
    }
    return null;
  }
  document(node) {
    const shouldWarnAboutMoreDocuments = this.processedDocumentCount === 1;
    const shouldSkipVisitingMoreDocuments = this.processedDocumentCount >= 1;
    if (shouldWarnAboutMoreDocuments) {
      const message = 'Only first document within YAML stream will be used. Rest will be discarded.';
      const element = new AnnotationElement(message);
      element.classes.push('warning');
      this.maybeAddSourceMap(node, element);
      this.annotations.push(element);
    }
    if (shouldSkipVisitingMoreDocuments) {
      return null;
    }
    this.processedDocumentCount += 1;
    return node.children;
  }
  mapping(node) {
    const element = new ObjectElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  keyValuePair(node) {
    const element = new MemberElement();

    // @ts-ignore
    element.content.key = node.key;
    // @ts-ignore
    element.content.value = node.value;
    this.maybeAddSourceMap(node, element);

    // process possible errors here that may be present in property node children as we're using direct field access
    node.children
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    .filter(child => child.type === 'error')
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    .forEach(errorNode => {
      this.error(errorNode, node, [], [node]);
    });
    return element;
  }
  sequence(node) {
    const element = new ArrayElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  scalar(node) {
    const element = this.namespace.toElement(node.content);

    // translate style information about empty nodes
    if (node.content === '' && node.style === YamlStyle.Plain) {
      element.classes.push('yaml-e-node');
      element.classes.push('yaml-e-scalar');
    }
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
    const message = node.isUnexpected ? '(Unexpected YAML syntax error)' : '(Error YAML syntax error)';
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
export default YamlAstVisitor;