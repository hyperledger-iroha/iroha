import { isNodeType } from "../../predicates.mjs";
/**
 * @public
 */
export const isStream = node => isNodeType('stream', node);

/**
 * @public
 */
export const isDocument = node => isNodeType('document', node);

/**
 * @public
 */
export const isMapping = node => isNodeType('mapping', node);

/**
 * @public
 */
export const isSequence = node => isNodeType('sequence', node);

/**
 * @public
 */
export const isKeyValuePair = node => isNodeType('keyValuePair', node);

/**
 * @public
 */
export const isTag = node => isNodeType('tag', node);

/**
 * @public
 */
export const isAnchor = node => isNodeType('anchor', node);

/**
 * @public
 */
export const isScalar = node => isNodeType('scalar', node);

/**
 * @public
 */
export const isAlias = node => isNodeType('alias', node);

/**
 * @public
 */
export const isDirective = node => isNodeType('directive', node);

/**
 * @public
 */
export const isComment = node => isNodeType('comment', node);