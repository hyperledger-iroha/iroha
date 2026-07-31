import { T as stubTrue } from 'ramda';
import { visit as astVisit, keyMapDefault as baseKeyMap, getNodeType as baseGetNodeType } from "../../../traversal/visitor.mjs";
const nodeTypeGetter = node => {
  if (typeof (node === null || node === void 0 ? void 0 : node.type) === 'string') {
    return node.type;
  }
  return baseGetNodeType(node);
};
const keyMapDefault = {
  EphemeralObject: ['content'],
  EphemeralArray: ['content'],
  ...baseKeyMap
};

// eslint-disable-next-line import/prefer-default-export
export const visit = (root,
// @ts-ignore
visitor, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  return astVisit(root, visitor, {
    keyMap,
    // @ts-ignore
    nodeTypeGetter,
    nodePredicate: stubTrue,
    detectCycles: false,
    deleteNodeSymbol: Symbol.for('delete-node'),
    skipVisitingNodeSymbol: Symbol.for('skip-visiting-node'),
    ...rest
  });
};

// @ts-ignore
visit[Symbol.for('nodejs.util.promisify.custom')] = async (root, {
  keyMap = keyMapDefault,
  ...rest
} = {}) => {
  // @ts-ignore
  return astVisit[Symbol.for('nodejs.util.promisify.custom')](root, visitor, {
    keyMap,
    nodeTypeGetter,
    nodePredicate: stubTrue,
    detectCycles: false,
    deleteNodeSymbol: Symbol.for('delete-node'),
    skipVisitingNodeSymbol: Symbol.for('skip-visiting-node'),
    ...rest
  });
};