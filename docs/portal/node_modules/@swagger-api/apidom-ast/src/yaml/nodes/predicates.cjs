"use strict";

exports.__esModule = true;
exports.isTag = exports.isStream = exports.isSequence = exports.isScalar = exports.isMapping = exports.isKeyValuePair = exports.isDocument = exports.isDirective = exports.isComment = exports.isAnchor = exports.isAlias = void 0;
var _predicates = require("../../predicates.cjs");
/**
 * @public
 */
const isStream = node => (0, _predicates.isNodeType)('stream', node);

/**
 * @public
 */
exports.isStream = isStream;
const isDocument = node => (0, _predicates.isNodeType)('document', node);

/**
 * @public
 */
exports.isDocument = isDocument;
const isMapping = node => (0, _predicates.isNodeType)('mapping', node);

/**
 * @public
 */
exports.isMapping = isMapping;
const isSequence = node => (0, _predicates.isNodeType)('sequence', node);

/**
 * @public
 */
exports.isSequence = isSequence;
const isKeyValuePair = node => (0, _predicates.isNodeType)('keyValuePair', node);

/**
 * @public
 */
exports.isKeyValuePair = isKeyValuePair;
const isTag = node => (0, _predicates.isNodeType)('tag', node);

/**
 * @public
 */
exports.isTag = isTag;
const isAnchor = node => (0, _predicates.isNodeType)('anchor', node);

/**
 * @public
 */
exports.isAnchor = isAnchor;
const isScalar = node => (0, _predicates.isNodeType)('scalar', node);

/**
 * @public
 */
exports.isScalar = isScalar;
const isAlias = node => (0, _predicates.isNodeType)('alias', node);

/**
 * @public
 */
exports.isAlias = isAlias;
const isDirective = node => (0, _predicates.isNodeType)('directive', node);

/**
 * @public
 */
exports.isDirective = isDirective;
const isComment = node => (0, _predicates.isNodeType)('comment', node);
exports.isComment = isComment;