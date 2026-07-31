import YamlTagError from "../../errors/YamlTagError.mjs";
import { YamlNodeKind } from "../../nodes/YamlTag.mjs";
import GenericMappingTag from "./GenericMapping.mjs";
import GenericSequenceTag from "./GenericSequence.mjs";
import GenericStringTag from "./GenericString.mjs";
import ScalarTag from "../ScalarTag.mjs";
/**
 * @public
 */
class FailsafeSchema {
  tags;
  tagDirectives;
  constructor() {
    this.tags = [];
    this.tagDirectives = [];
    this.registerTag(new GenericMappingTag());
    this.registerTag(new GenericSequenceTag());
    this.registerTag(new GenericStringTag());
  }

  // eslint-disable-next-line class-methods-use-this
  toSpecificTagName(node) {
    let specificTagName = node.tag.explicitName;
    if (node.tag.explicitName === '!') {
      // non-specific tag; we assume tag by kind
      if (node.tag.kind === YamlNodeKind.Scalar) {
        specificTagName = GenericStringTag.uri;
      } else if (node.tag.kind === YamlNodeKind.Sequence) {
        specificTagName = GenericSequenceTag.uri;
      } else if (node.tag.kind === YamlNodeKind.Mapping) {
        specificTagName = GenericMappingTag.uri;
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
    if (ScalarTag.test(node)) {
      canonicalNode = ScalarTag.canonicalFormat(node);
    }
    const tag = this.tags.find(itag => (itag === null || itag === void 0 ? void 0 : itag.tag) === specificTagName);

    // mechanism for resolving node (tag implementation) not found
    if (typeof tag === 'undefined') {
      throw new YamlTagError(`Tag "${specificTagName}" was not recognized.`, {
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
      throw new YamlTagError(`Node couldn't be resolved against the tag "${specificTagName}"`, {
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
export default FailsafeSchema;