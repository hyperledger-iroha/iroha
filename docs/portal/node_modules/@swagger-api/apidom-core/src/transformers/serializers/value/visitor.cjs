"use strict";

exports.__esModule = true;
exports.visit = void 0;
var _ramda = require("ramda");
var _visitor = require("../../../traversal/visitor.cjs");
const nodeTypeGetter = node => {
  if (typeof (node == null ? void 0 : node.type) === 'string') {
    return node.type;
  }
  return (0, _visitor.getNodeType)(node);
};
const keyMapDefault = {
  EphemeralObject: ['content'],
  EphemeralArray: ['content'],
  ..._visitor.keyMapDefault
};

// eslint-disable-next-line import/prefer-default-export
const visit = (root,
// @ts-ignore
visitor, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  return (0, _visitor.visit)(root, visitor, {
    keyMap,
    // @ts-ignore
    nodeTypeGetter,
    nodePredicate: _ramda.T,
    detectCycles: false,
    deleteNodeSymbol: Symbol.for('delete-node'),
    skipVisitingNodeSymbol: Symbol.for('skip-visiting-node'),
    ...rest
  });
};

// @ts-ignore
exports.visit = visit;
visit[Symbol.for('nodejs.util.promisify.custom')] = async (root, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  // @ts-ignore
  return _visitor.visit[Symbol.for('nodejs.util.promisify.custom')](root, visitor, {
    keyMap,
    nodeTypeGetter,
    nodePredicate: _ramda.T,
    detectCycles: false,
    deleteNodeSymbol: Symbol.for('delete-node'),
    skipVisitingNodeSymbol: Symbol.for('skip-visiting-node'),
    ...rest
  });
};