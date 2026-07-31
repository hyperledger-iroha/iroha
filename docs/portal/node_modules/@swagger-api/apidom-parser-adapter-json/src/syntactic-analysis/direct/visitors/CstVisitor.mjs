import { BooleanElement, NullElement, NumberElement, ParseResultElement, MemberElement, ObjectElement, ArrayElement, StringElement, AnnotationElement, isPrimitiveElement, assignSourceMap } from '@swagger-api/apidom-core';
import TreeCursorSyntaxNode from "../../TreeCursorSyntaxNode.mjs";
/* eslint-disable no-underscore-dangle */
class CstVisitor {
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
  enter(node) {
    // missing anonymous literals from CST transformed into AnnotationElements.
    if (node instanceof TreeCursorSyntaxNode && !node.isNamed && node.isMissing) {
      // collect annotations from missing literals
      const value = node.type || node.text;
      const message = `(Missing ${value})`;
      const element = new AnnotationElement(message);
      element.classes.push('warning');
      this.maybeAddSourceMap(node, element);
      this.annotations.push(element);
    }
    return null; // remove everything unrecognized
  }
  document(node) {
    const element = new ParseResultElement();
    // @ts-ignore
    element._content = node.children;
    this.maybeAddSourceMap(node, element);
    return element;
  }
  object(node) {
    const element = new ObjectElement();
    // @ts-ignore
    element._content = node.children;
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
  pair(node) {
    const element = new MemberElement();
    // @ts-ignore
    element.content.key = node.keyNode;
    // @ts-ignore
    element.content.value = node.valueNode;
    this.maybeAddSourceMap(node, element);

    /**
     * Process possible errors here that may be present in pair node children as we're using direct field access.
     * There are usually 3 children here found: "key", ":", "value".
     */
    if (node.children.length > 3) {
      node.children.filter(child => child.type === 'ERROR').forEach(errorNode => {
        this.ERROR(errorNode, node, [], [node]);
      });
    }
    return element;
  }
  string(node) {
    const element = new StringElement(JSON.parse(node.text));
    this.maybeAddSourceMap(node, element);
    return element;
  }
  number(node) {
    const element = new NumberElement(Number(node.text));
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
  ERROR(node, key, parent, path) {
    // collect errors as annotations
    const isUnexpected = !node.hasError;
    const value = node.text;
    const message = isUnexpected ? `(Unexpected ${value})` : `(Error ${value})`;
    const element = new AnnotationElement(message);
    element.classes.push('error');
    this.maybeAddSourceMap(node, element);
    if (path.length === 0) {
      // no document to visit, only error is present in CST
      const parseResultElement = new ParseResultElement();
      parseResultElement.push(element);
      return parseResultElement;
    }

    // we have CST node for document
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

/* eslint-enable no-underscore-dangle */

export default CstVisitor;