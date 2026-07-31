"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomAst = require("@swagger-api/apidom-ast");
var _TreeCursorIterator = _interopRequireDefault(require("../TreeCursorIterator.cjs"));
var _CstVisitor = _interopRequireWildcard(require("./visitors/CstVisitor.cjs"));
var _JsonAstVisitor = _interopRequireWildcard(require("./visitors/JsonAstVisitor.cjs"));
/**
 * @public
 */

/**
 * This version of syntactic analysis does following transformations:
 *   `TreeSitter CST -> JSON AST -> ApiDOM`
 *
 * Transient transformation of TreeSitter CST is performed
 * using TreeSitter cursor. TreeSitter cursor is a stateful object
 * that allows us to walk syntax tree containing large number of nodes
 * with maximum efficiency. Using this transient CST transformation
 * gives us double the performance when syntactically analyzing
 * CST into JSON AST.
 *
 * Two traversals passes are needed to get from CST to ApiDOM.
 * This analysis is much slower than the direct one, but allows
 * to do additional analysis magic on JSON AST.
 * @public
 */
const analyze = (cst, {
  sourceMap = false
} = {}) => {
  const cursor = cst.walk();
  const iterator = new _TreeCursorIterator.default(cursor);
  const [rootNode] = Array.from(iterator);
  const cstVisitor = new _CstVisitor.default();
  const astVisitor = new _JsonAstVisitor.default();
  const jsonAst = (0, _apidomAst.visit)(rootNode, cstVisitor, {
    // @ts-ignore
    keyMap: _CstVisitor.keyMap,
    state: {
      sourceMap
    }
  });
  return (0, _apidomAst.visit)(jsonAst.rootNode, astVisitor, {
    // @ts-ignore
    keyMap: _JsonAstVisitor.keyMap,
    nodeTypeGetter: _JsonAstVisitor.getNodeType,
    nodePredicate: _JsonAstVisitor.isNode,
    state: {
      sourceMap
    }
  });
};
var _default = exports.default = analyze;