import { Error, isNode as isCSTNode, Literal, ParseResult, YamlAlias, YamlAnchor, YamlComment, YamlDirective, YamlDocument, YamlKeyValuePair, YamlMapping, YamlNodeKind, YamlScalar, YamlSequence, YamlStream, YamlStyle, YamlStyleGroup, YamlTag } from '@swagger-api/apidom-ast';
import { assignSourceMap } from '@swagger-api/apidom-core';
import TreeCursorSyntaxNode from "../../TreeCursorSyntaxNode.mjs";
export const keyMap = {
  stream: ['children'],
  document: ['children'],
  mapping: ['children'],
  keyValuePair: ['children'],
  sequence: ['children'],
  error: ['children']
};
export const isNode = node => Array.isArray(node) || isCSTNode(node);

/* eslint-disable no-param-reassign */

class CstVisitor {
  static isScalar = this.isKind('scalar');
  static isMapping = this.isKind('mapping');
  static isSequence = this.isKind('sequence');
  static isKind(ending) {
    return node => node != null && typeof node === 'object' && 'type' in node && typeof node.type === 'string' && node.type.endsWith(ending);
  }
  static kindNodeToYamlAnchor(node) {
    const {
      anchor: anchorNode
    } = node;
    if (typeof anchorNode === 'undefined') return undefined;
    return new YamlAnchor(assignSourceMap({
      name: anchorNode.text
    }, anchorNode));
  }
  static hasKeyValuePairEmptyKey(node) {
    if (node.type !== 'block_mapping_pair' && node.type !== 'flow_pair') {
      return false;
    }
    // keyNode was not explicitly provided; tag and anchor are missing too
    return typeof node.keyNode === 'undefined';
  }
  static hasKeyValuePairEmptyValue(node) {
    if (node.type !== 'block_mapping_pair' && node.type !== 'flow_pair') {
      return false;
    }
    // valueNode was not explicitly provided; tag and anchor are missing too
    return typeof node.valueNode === 'undefined';
  }
  static kindNodeToYamlTag(node) {
    const {
      tag: tagNode
    } = node;
    const explicitName = (tagNode === null || tagNode === void 0 ? void 0 : tagNode.text) || (node.type === 'plain_scalar' ? '?' : '!');
    const kind = node.type.endsWith('mapping') ? YamlNodeKind.Mapping : node.type.endsWith('sequence') ? YamlNodeKind.Sequence : YamlNodeKind.Scalar;
    return new YamlTag(assignSourceMap({
      explicitName,
      kind
    }, tagNode));
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  schema;
  referenceManager;
  stream = {
    enter: node => {
      return new YamlStream(assignSourceMap({
        children: node.children,
        isMissing: node.isMissing
      }, node));
    },
    leave: stream => {
      return new ParseResult({
        children: [stream]
      });
    }
  };
  yaml_directive = {
    enter: node => {
      var _node$firstNamedChild;
      const version = node === null || node === void 0 || (_node$firstNamedChild = node.firstNamedChild) === null || _node$firstNamedChild === void 0 ? void 0 : _node$firstNamedChild.text;
      return new YamlDirective(assignSourceMap({
        name: '%YAML',
        parameters: {
          version
        }
      }, node));
    }
  };
  tag_directive = {
    enter: node => {
      const tagHandleNode = node.children[0];
      const tagPrefixNode = node.children[1];
      const tagDirective = new YamlDirective(assignSourceMap({
        name: '%TAG',
        parameters: {
          handle: tagHandleNode === null || tagHandleNode === void 0 ? void 0 : tagHandleNode.text,
          prefix: tagPrefixNode === null || tagPrefixNode === void 0 ? void 0 : tagPrefixNode.text
        }
      }, node));
      this.schema.registerTagDirective(tagDirective);
      return tagDirective;
    }
  };
  reserved_directive = {
    enter: node => {
      const directiveNameNode = node.children[0];
      const directiveParameter1Node = node.children[1];
      const directiveParameter2Node = node.children[2];
      return new YamlDirective(assignSourceMap({
        name: directiveNameNode === null || directiveNameNode === void 0 ? void 0 : directiveNameNode.text,
        parameters: {
          handle: directiveParameter1Node === null || directiveParameter1Node === void 0 ? void 0 : directiveParameter1Node.text,
          prefix: directiveParameter2Node === null || directiveParameter2Node === void 0 ? void 0 : directiveParameter2Node.text
        }
      }, node));
    }
  };
  document = {
    enter: node => {
      return new YamlDocument(assignSourceMap({
        children: node.children,
        isMissing: node.isMissing
      }, node));
    },
    leave: node => {
      node.children = node.children.flat();
    }
  };
  block_node = {
    enter: node => {
      return node.children;
    }
  };
  flow_node = {
    enter: node => {
      const [kindCandidate] = node.children.slice(-1);

      // kind node is present in flow node
      if (CstVisitor.isScalar(kindCandidate) || CstVisitor.isMapping(kindCandidate) || CstVisitor.isSequence(kindCandidate)) {
        return node.children;
      }
      // kind node not present in flow node, creating empty node
      const emptyScalarNode = new YamlScalar({
        content: '',
        anchor: CstVisitor.kindNodeToYamlAnchor(kindCandidate),
        tag: CstVisitor.kindNodeToYamlTag(kindCandidate),
        startPositionRow: kindCandidate.endPositionRow,
        startPositionColumn: kindCandidate.endPositionColumn,
        startIndex: kindCandidate.endIndex,
        endPositionRow: kindCandidate.endPositionRow,
        endPositionColumn: kindCandidate.endPositionColumn,
        endIndex: kindCandidate.endIndex,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.Plain
      });
      this.registerAnchor(emptyScalarNode);
      return [...node.children, emptyScalarNode];
    }
  };
  tag = {
    enter: () => {
      return null;
    }
  };
  anchor = {
    enter: () => {
      return null;
    }
  };
  block_mapping = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const mappingNode = new YamlMapping(assignSourceMap({
        children: node.children,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Block,
        style: YamlStyle.NextLine,
        isMissing: node.isMissing
      }, node));
      this.registerAnchor(mappingNode);
      return this.schema.resolve(mappingNode);
    }
  };
  block_mapping_pair = {
    enter: node => {
      const children = [...node.children];
      if (CstVisitor.hasKeyValuePairEmptyKey(node)) {
        const keyNode = this.createKeyValuePairEmptyKey(node);
        children.unshift(keyNode);
      }
      if (CstVisitor.hasKeyValuePairEmptyValue(node)) {
        const valueNode = this.createKeyValuePairEmptyValue(node);
        children.push(valueNode);
      }
      return new YamlKeyValuePair(assignSourceMap({
        children,
        styleGroup: YamlStyleGroup.Block,
        isMissing: node.isMissing
      }, node));
    }
  };
  flow_mapping = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const mappingNode = new YamlMapping(assignSourceMap({
        children: node.children,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.Explicit,
        isMissing: node.isMissing
      }, node));
      this.registerAnchor(mappingNode);
      return this.schema.resolve(mappingNode);
    }
  };
  flow_pair = {
    enter: node => {
      const children = [...node.children];
      if (CstVisitor.hasKeyValuePairEmptyKey(node)) {
        const keyNode = this.createKeyValuePairEmptyKey(node);
        children.unshift(keyNode);
      }
      if (CstVisitor.hasKeyValuePairEmptyValue(node)) {
        const valueNode = this.createKeyValuePairEmptyValue(node);
        children.push(valueNode);
      }
      return new YamlKeyValuePair(assignSourceMap({
        children,
        styleGroup: YamlStyleGroup.Flow,
        isMissing: node.isMissing
      }, node));
    }
  };
  keyValuePair = {
    leave: node => {
      node.children = node.children.flat();
    }
  };
  block_sequence = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const sequenceNode = new YamlSequence(assignSourceMap({
        children: node.children,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Block,
        style: YamlStyle.NextLine
      }, node));
      this.registerAnchor(sequenceNode);
      return this.schema.resolve(sequenceNode);
    }
  };
  block_sequence_item = {
    enter: node => {
      // flow or block node present; first node is always `-` literal
      if (node.children.length > 1) {
        return node.children;
      }

      // create empty node
      const emptyScalarNode = new YamlScalar({
        content: '',
        tag: new YamlTag({
          explicitName: '?',
          kind: YamlNodeKind.Scalar
        }),
        startPositionRow: node.endPositionRow,
        startPositionColumn: node.endPositionColumn,
        startIndex: node.endIndex,
        endPositionRow: node.endPositionRow,
        endPositionColumn: node.endPositionColumn,
        endIndex: node.endIndex,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.Plain
      });
      return [emptyScalarNode];
    }
  };
  flow_sequence = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const sequenceNode = new YamlSequence(assignSourceMap({
        children: node.children.flat(),
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.Explicit
      }, node));
      this.registerAnchor(sequenceNode);
      return this.schema.resolve(sequenceNode);
    }
  };
  sequence = {
    leave: node => {
      node.children = node.children.flat(+Infinity);
    }
  };
  plain_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const scalarNode = new YamlScalar(assignSourceMap({
        content: node.text,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.Plain
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  single_quote_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const scalarNode = new YamlScalar(assignSourceMap({
        content: node.text,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.SingleQuoted
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  double_quote_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const scalarNode = new YamlScalar(assignSourceMap({
        content: node.text,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Flow,
        style: YamlStyle.DoubleQuoted
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  block_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const style = node.text.startsWith('|') ? YamlStyle.Literal : node.text.startsWith('>') ? YamlStyle.Folded : YamlStyle.Plain;
      const scalarNode = new YamlScalar(assignSourceMap({
        content: node.text,
        anchor,
        tag,
        styleGroup: YamlStyleGroup.Block,
        style
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  comment = {
    enter: node => {
      return new YamlComment({
        content: node.text
      });
    }
  };
  alias = {
    enter: node => {
      const alias = new YamlAlias({
        content: node.text
      });
      return this.referenceManager.resolveAlias(alias);
    }
  };

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  constructor(schema) {
    this.schema = schema;
  }

  // eslint-disable-next-line class-methods-use-this
  enter(node) {
    // missing anonymous literals from CST transformed into AST literal nodes
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

  // eslint-disable-next-line class-methods-use-this
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
  registerAnchor(node) {
    if (node.anchor !== undefined) {
      this.referenceManager.addAnchor(node);
    }
  }
  createKeyValuePairEmptyKey(node) {
    const {
      keyNode
    } = node;
    const children = (keyNode === null || keyNode === void 0 ? void 0 : keyNode.children) || [];
    const tagNode = children.find(CstVisitor.isKind('tag'));
    const anchorNode = children.find(CstVisitor.isKind('anchor'));
    const tag = typeof tagNode !== 'undefined' ? new YamlTag(assignSourceMap({
      explicitName: tagNode.text,
      kind: YamlNodeKind.Scalar
    }, tagNode)) : new YamlTag({
      explicitName: '?',
      kind: YamlNodeKind.Scalar
    });
    const anchor = typeof anchorNode !== 'undefined' ? new YamlAnchor(assignSourceMap({
      name: anchorNode.text
    }, anchorNode)) : undefined;
    const scalarNode = new YamlScalar({
      content: '',
      startPositionRow: node.startPositionRow,
      startPositionColumn: node.startPositionColumn,
      startIndex: node.startIndex,
      endPositionRow: node.startPositionRow,
      endPositionColumn: node.startPositionColumn,
      endIndex: node.startIndex,
      tag,
      anchor,
      styleGroup: YamlStyleGroup.Flow,
      style: YamlStyle.Plain
    });
    this.registerAnchor(scalarNode);
    return scalarNode;
  }
  createKeyValuePairEmptyValue(node) {
    const {
      valueNode
    } = node;
    const children = (valueNode === null || valueNode === void 0 ? void 0 : valueNode.children) || [];
    const tagNode = children.find(CstVisitor.isKind('tag'));
    const anchorNode = children.find(CstVisitor.isKind('anchor'));
    const tag = typeof tagNode !== 'undefined' ? new YamlTag(assignSourceMap({
      explicitName: tagNode.text,
      kind: YamlNodeKind.Scalar
    }, tagNode)) : new YamlTag({
      explicitName: '?',
      kind: YamlNodeKind.Scalar
    });
    const anchor = typeof anchorNode !== 'undefined' ? new YamlAnchor(assignSourceMap({
      name: anchorNode.text
    }, anchorNode)) : undefined;
    const scalarNode = new YamlScalar({
      content: '',
      startPositionRow: node.endPositionRow,
      startPositionColumn: node.endPositionColumn,
      startIndex: node.endIndex,
      endPositionRow: node.endPositionRow,
      endPositionColumn: node.endPositionColumn,
      endIndex: node.endIndex,
      tag,
      anchor,
      styleGroup: YamlStyleGroup.Flow,
      style: YamlStyle.Plain
    });
    this.registerAnchor(scalarNode);
    return scalarNode;
  }
}
export default CstVisitor;