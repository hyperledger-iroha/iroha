"use strict";

exports.__esModule = true;
exports.isParseResult = exports.isNodeType = exports.isLiteral = void 0;
/**
 * @public
 */
const isNodeType = (type, node) => node != null && typeof node === 'object' && 'type' in node && node.type === type;

/**
 * @public
 */
exports.isNodeType = isNodeType;
const isLiteral = node => isNodeType('literal', node);

/**
 * @public
 */
exports.isLiteral = isLiteral;
const isParseResult = node => isNodeType('parseResult', node);
exports.isParseResult = isParseResult;