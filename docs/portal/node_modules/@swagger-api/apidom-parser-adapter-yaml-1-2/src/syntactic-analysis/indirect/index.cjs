"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomAst = require("@swagger-api/apidom-ast");
var _CstVisitor = _interopRequireWildcard(require("./visitors/CstVisitor.cjs"));
var _YamlAstVisitor = _interopRequireWildcard(require("./visitors/YamlAstVisitor.cjs"));
var _TreeCursorIterator = _interopRequireDefault(require("../TreeCursorIterator.cjs"));
/**
 * @public
 */

/**
 * This version of syntactic analysis does following transformations:
 *   `TreeSitter CST -> YAML AST -> ApiDOM`
 * Two traversals passes are needed to get from CST to ApiDOM.
 * @public
 */
const analyze = (cst, {
  sourceMap = false
} = {}) => {
  const cursor = cst.walk();
  const iterator = new _TreeCursorIterator.default(cursor);
  const [rootNode] = Array.from(iterator);
  const cstVisitor = new _CstVisitor.default();
  const astVisitor = new _YamlAstVisitor.default();
  const schema = new _apidomAst.YamlJsonSchema();
  const referenceManager = new _apidomAst.YamlReferenceManager();
  const yamlAst = (0, _apidomAst.visit)(rootNode, cstVisitor, {
    // @ts-ignore
    keyMap: _CstVisitor.keyMap,
    nodePredicate: _CstVisitor.isNode,
    state: {
      schema,
      sourceMap,
      referenceManager
    }
  });
  return (0, _apidomAst.visit)(yamlAst.rootNode, astVisitor, {
    // @ts-ignore
    keyMap: _YamlAstVisitor.keyMap,
    nodeTypeGetter: _YamlAstVisitor.getNodeType,
    nodePredicate: _YamlAstVisitor.isNode,
    state: {
      sourceMap
    }
  });
};
var _default = exports.default = analyze;