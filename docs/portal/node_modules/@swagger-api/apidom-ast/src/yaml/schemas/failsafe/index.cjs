"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _YamlTagError = _interopRequireDefault(require("../../errors/YamlTagError.cjs"));
var _YamlTag = require("../../nodes/YamlTag.cjs");
var _GenericMapping = _interopRequireDefault(require("./GenericMapping.cjs"));
var _GenericSequence = _interopRequireDefault(require("./GenericSequence.cjs"));
var _GenericString = _interopRequireDefault(require("./GenericString.cjs"));
var _ScalarTag = _interopRequireDefault(require("../ScalarTag.cjs"));
/**
 * @public
 */
class FailsafeSchema {
  tags;
  tagDirectives;
  constructor() {
    this.tags = [];
    this.tagDirectives = [];
    this.registerTag(new _GenericMapping.default());
    this.registerTag(new _GenericSequence.default());
    this.registerTag(new _GenericString.default());
  }

  // eslint-disable-next-line class-methods-use-this
  toSpecificTagName(node) {
    let specificTagName = node.tag.explicitName;
    if (node.tag.explicitName === '!') {
      // non-specific tag; we assume tag by kind
      if (node.tag.kind === _YamlTag.YamlNodeKind.Scalar) {
        specificTagName = _GenericString.default.uri;
      } else if (node.tag.kind === _YamlTag.YamlNodeKind.Sequence) {
        specificTagName = _GenericSequence.default.uri;
      } else if (node.tag.kind === _YamlTag.YamlNodeKind.Mapping) {
        specificTagName = _GenericMapping.default.uri;
      }
    } else if (node.tag.explicitName.startsWith('!<')) {
      // verbatim form
      specificTagName = node.tag.explicitName.replace(/^!</, '').replace(/>$/, '');
    } else if (node.tag.explicitName.startsWith('!!')) {
      // shorthand notation
      specificTagName = `tag:yaml.org,2002:${node.tag.explicitName.replace(/^!!/, '')}`;
    }
    return specificTagName;
  }
  registerTagDirective(tagDirective) {
    this.tagDirectives.push({
      // @ts-ignore
      handle: tagDirective.parameters.handle,
      // @ts-ignore
      prefix: tagDirective.parameters.prefix
    });
  }
  registerTag(tag, beginning = false) {
    if (beginning) {
      this.tags.unshift(tag);
    } else {
      this.tags.push(tag);
    }
    return this;
  }
  overrideTag(tag) {
    this.tags = this.tags.filter(itag => itag.tag === tag.tag);
    this.tags.push(tag);
    return this;
  }
  resolve(node) {
    const specificTagName = this.toSpecificTagName(node);

    // leave this node unresolved
    if (specificTagName === '?') {
      return node;
    }

    // turn scalar nodes into canonical format before resolving
    let canonicalNode = node;
    if (_ScalarTag.default.test(node)) {
      canonicalNode = _ScalarTag.default.canonicalFormat(node);
    }
    const tag = this.tags.find(itag => (itag == null ? void 0 : itag.tag) === specificTagName);

    // mechanism for resolving node (tag implementation) not found
    if (typeof tag === 'undefined') {
      throw new _YamlTagError.default(`Tag "${specificTagName}" was not recognized.`, {
        specificTagName,
        explicitTagName: node.tag.explicitName,
        tagKind: node.tag.kind,
        tagStartPositionRow: node.tag.startPositionRow,
        tagStartPositionColumn: node.tag.startPositionColumn,
        tagStartPositionIndex: node.tag.startIndex,
        tagEndPositionRow: node.tag.endPositionRow,
        tagEndPositionColumn: node.tag.endPositionColumn,
        tagEndPositionIndex: node.tag.endIndex,
        node: node.clone()
      });
    }

    // node content is not compatible with resolving mechanism (tag implementation)
    if (!tag.test(canonicalNode)) {
      throw new _YamlTagError.default(`Node couldn't be resolved against the tag "${specificTagName}"`, {
        specificTagName,
        explicitTagName: node.tag.explicitName,
        tagKind: node.tag.kind,
        tagStartPositionRow: node.tag.startPositionRow,
        tagStartPositionColumn: node.tag.startPositionColumn,
        tagStartPositionIndex: node.tag.startIndex,
        tagEndPositionRow: node.tag.endPositionRow,
        tagEndPositionColumn: node.tag.endPositionColumn,
        tagEndPositionIndex: node.tag.endIndex,
        nodeCanonicalContent: canonicalNode.content,
        node: node.clone()
      });
    }
    return tag.resolve(canonicalNode);
  }
}
var _default = exports.default = FailsafeSchema;