"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomAst = require("@swagger-api/apidom-ast");
var _apidomCore = require("@swagger-api/apidom-core");
var _CstVisitor = _interopRequireDefault(require("./visitors/CstVisitor.cjs"));
var _TreeCursorIterator = _interopRequireDefault(require("../TreeCursorIterator.cjs"));
const keyMap = {
  document: ['children'],
  object: ['children'],
  array: ['children'],
  string: ['children'],
  property: ['children'],
  key: ['children'],
  error: ['children'],
  ..._apidomCore.keyMap
};
const getNodeType = node => {
  if ((0, _apidomCore.isParseResultElement)(node)) {
    return 'ParseResultElement';
  }
  if ((0, _apidomCore.isElement)(node)) {
    return (0, _apidomCore.getNodeType)(node);
  }
  return (0, _apidomAst.getNodeType)(node);
};

// @ts-ignore
const isNode = element => (0, _apidomCore.isElement)(element) || (0, _apidomAst.isNode)(element);

/**
 * This version of syntactic analysis translates TreeSitter CTS
 * directly into ApiDOM.
 *
 * Transient transformation of TreeSitter CST is performed
 * using TreeSitter cursor. TreeSitter cursor is a stateful object
 * that allows us to walk syntax tree containing large number of nodes
 * with maximum efficiency. Using this transient CST transformation
 * gives us double the performance when syntactically analyzing
 * CST into ApiDOM.
 *
 * Single traversal pass is needed to get from CST to ApiDOM.
 * @public
 */
const analyze = (cst, {
  sourceMap = false
} = {}) => {
  const visitor = new _CstVisitor.default();
  const cursor = cst.walk();
  const iterator = new _TreeCursorIterator.default(cursor);
  const [rootNode] = Array.from(iterator);
  return (0, _apidomAst.visit)(rootNode, visitor, {
    // @ts-ignore
    keyMap,
    nodeTypeGetter: getNodeType,
    nodePredicate: isNode,
    state: {
      sourceMap
    }
  });
};
var _default = exports.default = analyze;