/**
 * @public
 */
export const isNodeType = (type, node) => node != null && typeof node === 'object' && 'type' in node && node.type === type;

/**
 * @public
 */
export const isLiteral = node => isNodeType('literal', node);

/**
 * @public
 */
export const isParseResult = node => isNodeType('parseResult', node);