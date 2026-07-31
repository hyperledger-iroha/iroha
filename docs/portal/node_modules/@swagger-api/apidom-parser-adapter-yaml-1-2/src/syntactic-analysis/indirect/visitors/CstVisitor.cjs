"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.keyMap = exports.isNode = exports.default = void 0;
var _apidomAst = require("@swagger-api/apidom-ast");
var _apidomCore = require("@swagger-api/apidom-core");
var _TreeCursorSyntaxNode = _interopRequireDefault(require("../../TreeCursorSyntaxNode.cjs"));
const keyMap = exports.keyMap = {
  stream: ['children'],
  document: ['children'],
  mapping: ['children'],
  keyValuePair: ['children'],
  sequence: ['children'],
  error: ['children']
};
const isNode = node => Array.isArray(node) || (0, _apidomAst.isNode)(node);

/* eslint-disable no-param-reassign */
exports.isNode = isNode;
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
    return new _apidomAst.YamlAnchor((0, _apidomCore.assignSourceMap)({
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
    const explicitName = (tagNode == null ? void 0 : tagNode.text) || (node.type === 'plain_scalar' ? '?' : '!');
    const kind = node.type.endsWith('mapping') ? _apidomAst.YamlNodeKind.Mapping : node.type.endsWith('sequence') ? _apidomAst.YamlNodeKind.Sequence : _apidomAst.YamlNodeKind.Scalar;
    return new _apidomAst.YamlTag((0, _apidomCore.assignSourceMap)({
      explicitName,
      kind
    }, tagNode));
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  schema;
  referenceManager;
  stream = {
    enter: node => {
      return new _apidomAst.YamlStream((0, _apidomCore.assignSourceMap)({
        children: node.children,
        isMissing: node.isMissing
      }, node));
    },
    leave: stream => {
      return new _apidomAst.ParseResult({
        children: [stream]
      });
    }
  };
  yaml_directive = {
    enter: node => {
      var _node$firstNamedChild;
      const version = node == null || (_node$firstNamedChild = node.firstNamedChild) == null ? void 0 : _node$firstNamedChild.text;
      return new _apidomAst.YamlDirective((0, _apidomCore.assignSourceMap)({
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
      const tagDirective = new _apidomAst.YamlDirective((0, _apidomCore.assignSourceMap)({
        name: '%TAG',
        parameters: {
          handle: tagHandleNode == null ? void 0 : tagHandleNode.text,
          prefix: tagPrefixNode == null ? void 0 : tagPrefixNode.text
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
      return new _apidomAst.YamlDirective((0, _apidomCore.assignSourceMap)({
        name: directiveNameNode == null ? void 0 : directiveNameNode.text,
        parameters: {
          handle: directiveParameter1Node == null ? void 0 : directiveParameter1Node.text,
          prefix: directiveParameter2Node == null ? void 0 : directiveParameter2Node.text
        }
      }, node));
    }
  };
  document = {
    enter: node => {
      return new _apidomAst.YamlDocument((0, _apidomCore.assignSourceMap)({
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
      const emptyScalarNode = new _apidomAst.YamlScalar({
        content: '',
        anchor: CstVisitor.kindNodeToYamlAnchor(kindCandidate),
        tag: CstVisitor.kindNodeToYamlTag(kindCandidate),
        startPositionRow: kindCandidate.endPositionRow,
        startPositionColumn: kindCandidate.endPositionColumn,
        startIndex: kindCandidate.endIndex,
        endPositionRow: kindCandidate.endPositionRow,
        endPositionColumn: kindCandidate.endPositionColumn,
        endIndex: kindCandidate.endIndex,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.Plain
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
      const mappingNode = new _apidomAst.YamlMapping((0, _apidomCore.assignSourceMap)({
        children: node.children,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Block,
        style: _apidomAst.YamlStyle.NextLine,
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
      return new _apidomAst.YamlKeyValuePair((0, _apidomCore.assignSourceMap)({
        children,
        styleGroup: _apidomAst.YamlStyleGroup.Block,
        isMissing: node.isMissing
      }, node));
    }
  };
  flow_mapping = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const mappingNode = new _apidomAst.YamlMapping((0, _apidomCore.assignSourceMap)({
        children: node.children,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.Explicit,
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
      return new _apidomAst.YamlKeyValuePair((0, _apidomCore.assignSourceMap)({
        children,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
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
      const sequenceNode = new _apidomAst.YamlSequence((0, _apidomCore.assignSourceMap)({
        children: node.children,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Block,
        style: _apidomAst.YamlStyle.NextLine
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
      const emptyScalarNode = new _apidomAst.YamlScalar({
        content: '',
        tag: new _apidomAst.YamlTag({
          explicitName: '?',
          kind: _apidomAst.YamlNodeKind.Scalar
        }),
        startPositionRow: node.endPositionRow,
        startPositionColumn: node.endPositionColumn,
        startIndex: node.endIndex,
        endPositionRow: node.endPositionRow,
        endPositionColumn: node.endPositionColumn,
        endIndex: node.endIndex,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.Plain
      });
      return [emptyScalarNode];
    }
  };
  flow_sequence = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const sequenceNode = new _apidomAst.YamlSequence((0, _apidomCore.assignSourceMap)({
        children: node.children.flat(),
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.Explicit
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
      const scalarNode = new _apidomAst.YamlScalar((0, _apidomCore.assignSourceMap)({
        content: node.text,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.Plain
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  single_quote_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const scalarNode = new _apidomAst.YamlScalar((0, _apidomCore.assignSourceMap)({
        content: node.text,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.SingleQuoted
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  double_quote_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const scalarNode = new _apidomAst.YamlScalar((0, _apidomCore.assignSourceMap)({
        content: node.text,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Flow,
        style: _apidomAst.YamlStyle.DoubleQuoted
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  block_scalar = {
    enter: node => {
      const tag = CstVisitor.kindNodeToYamlTag(node);
      const anchor = CstVisitor.kindNodeToYamlAnchor(node);
      const style = node.text.startsWith('|') ? _apidomAst.YamlStyle.Literal : node.text.startsWith('>') ? _apidomAst.YamlStyle.Folded : _apidomAst.YamlStyle.Plain;
      const scalarNode = new _apidomAst.YamlScalar((0, _apidomCore.assignSourceMap)({
        content: node.text,
        anchor,
        tag,
        styleGroup: _apidomAst.YamlStyleGroup.Block,
        style
      }, node));
      this.registerAnchor(scalarNode);
      return this.schema.resolve(scalarNode);
    }
  };
  comment = {
    enter: node => {
      return new _apidomAst.YamlComment({
        content: node.text
      });
    }
  };
  alias = {
    enter: node => {
      const alias = new _apidomAst.YamlAlias({
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

  // eslint-disable-next-line class-methods-use-this
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
  registerAnchor(node) {
    if (node.anchor !== undefined) {
      this.referenceManager.addAnchor(node);
    }
  }
  createKeyValuePairEmptyKey(node) {
    const {
      keyNode
    } = node;
    const children = (keyNode == null ? void 0 : keyNode.children) || [];
    const tagNode = children.find(CstVisitor.isKind('tag'));
    const anchorNode = children.find(CstVisitor.isKind('anchor'));
    const tag = typeof tagNode !== 'undefined' ? new _apidomAst.YamlTag((0, _apidomCore.assignSourceMap)({
      explicitName: tagNode.text,
      kind: _apidomAst.YamlNodeKind.Scalar
    }, tagNode)) : new _apidomAst.YamlTag({
      explicitName: '?',
      kind: _apidomAst.YamlNodeKind.Scalar
    });
    const anchor = typeof anchorNode !== 'undefined' ? new _apidomAst.YamlAnchor((0, _apidomCore.assignSourceMap)({
      name: anchorNode.text
    }, anchorNode)) : undefined;
    const scalarNode = new _apidomAst.YamlScalar({
      content: '',
      startPositionRow: node.startPositionRow,
      startPositionColumn: node.startPositionColumn,
      startIndex: node.startIndex,
      endPositionRow: node.startPositionRow,
      endPositionColumn: node.startPositionColumn,
      endIndex: node.startIndex,
      tag,
      anchor,
      styleGroup: _apidomAst.YamlStyleGroup.Flow,
      style: _apidomAst.YamlStyle.Plain
    });
    this.registerAnchor(scalarNode);
    return scalarNode;
  }
  createKeyValuePairEmptyValue(node) {
    const {
      valueNode
    } = node;
    const children = (valueNode == null ? void 0 : valueNode.children) || [];
    const tagNode = children.find(CstVisitor.isKind('tag'));
    const anchorNode = children.find(CstVisitor.isKind('anchor'));
    const tag = typeof tagNode !== 'undefined' ? new _apidomAst.YamlTag((0, _apidomCore.assignSourceMap)({
      explicitName: tagNode.text,
      kind: _apidomAst.YamlNodeKind.Scalar
    }, tagNode)) : new _apidomAst.YamlTag({
      explicitName: '?',
      kind: _apidomAst.YamlNodeKind.Scalar
    });
    const anchor = typeof anchorNode !== 'undefined' ? new _apidomAst.YamlAnchor((0, _apidomCore.assignSourceMap)({
      name: anchorNode.text
    }, anchorNode)) : undefined;
    const scalarNode = new _apidomAst.YamlScalar({
      content: '',
      startPositionRow: node.endPositionRow,
      startPositionColumn: node.endPositionColumn,
      startIndex: node.endIndex,
      endPositionRow: node.endPositionRow,
      endPositionColumn: node.endPositionColumn,
      endIndex: node.endIndex,
      tag,
      anchor,
      styleGroup: _apidomAst.YamlStyleGroup.Flow,
      style: _apidomAst.YamlStyle.Plain
    });
    this.registerAnchor(scalarNode);
    return scalarNode;
  }
}
var _default = exports.default = CstVisitor;