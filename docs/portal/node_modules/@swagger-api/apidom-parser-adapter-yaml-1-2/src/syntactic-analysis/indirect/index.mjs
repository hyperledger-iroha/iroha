import { visit, YamlJsonSchema as JsonSchema, YamlReferenceManager } from '@swagger-api/apidom-ast';
import CstVisitor, { keyMap as cstKeyMap, isNode as isCstNode } from "./visitors/CstVisitor.mjs";
import YamlAstVisitor, { keyMap as astKeyMap, isNode as isAstNode, getNodeType as getAstNodeType } from "./visitors/YamlAstVisitor.mjs";
import TreeCursorIterator from "../TreeCursorIterator.mjs";
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
  const iterator = new TreeCursorIterator(cursor);
  const [rootNode] = Array.from(iterator);
  const cstVisitor = new CstVisitor();
  const astVisitor = new YamlAstVisitor();
  const schema = new JsonSchema();
  const referenceManager = new YamlReferenceManager();
  const yamlAst = visit(rootNode, cstVisitor, {
    // @ts-ignore
    keyMap: cstKeyMap,
    nodePredicate: isCstNode,
    state: {
      schema,
      sourceMap,
      referenceManager
    }
  });
  return visit(yamlAst.rootNode, astVisitor, {
    // @ts-ignore
    keyMap: astKeyMap,
    nodeTypeGetter: getAstNodeType,
    nodePredicate: isAstNode,
    state: {
      sourceMap
    }
  });
};
export default analyze;