import { ApiDOMStructuredError } from '@swagger-api/apidom-error';

/**
 * SPDX-FileCopyrightText: Copyright (c) GraphQL Contributors
 *
 * SPDX-License-Identifier: MIT
 */

/**
 * @public
 */
export const getVisitFn = (visitor, type, isLeaving) => {
  // @ts-ignore
  const typeVisitor = visitor[type];
  if (typeVisitor != null) {
    if (!isLeaving && typeof typeVisitor === 'function') {
      // { Type() {} }
      return typeVisitor;
    }
    const typeSpecificVisitor = isLeaving ? typeVisitor.leave : typeVisitor.enter;
    if (typeof typeSpecificVisitor === 'function') {
      // { Type: { enter() {}, leave() {} } }
      return typeSpecificVisitor;
    }
  } else {
    const specificVisitor = isLeaving ? visitor.leave : visitor.enter;
    if (specificVisitor != null) {
      if (typeof specificVisitor === 'function') {
        // { enter() {}, leave() {} }
        return specificVisitor;
      }
      // @ts-ignore
      const specificTypeVisitor = specificVisitor[type];
      if (typeof specificTypeVisitor === 'function') {
        // { enter: { Type() {} }, leave: { Type() {} } }
        return specificTypeVisitor;
      }
    }
  }
  return null;
};

/**
 * @public
 */
export const BREAK = {};

/**
 * @public
 */
export const getNodeType = node => node === null || node === void 0 ? void 0 : node.type;

/**
 * @public
 */
export const isNode = node => typeof getNodeType(node) === 'string';

/**
 * @public
 */
export const cloneNode = node => Object.create(Object.getPrototypeOf(node), Object.getOwnPropertyDescriptors(node));

/**
 * Creates a new visitor instance which delegates to many visitors to run in
 * parallel. Each visitor will be visited for each node before moving on.
 *
 * If a prior visitor edits a node, no following visitors will see that node.
 * `exposeEdits=true` can be used to expose the edited node from the previous visitors.
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
export const mergeAll = (visitors, {
  visitFnGetter = getVisitFn,
  nodeTypeGetter = getNodeType,
  breakSymbol = BREAK,
  deleteNodeSymbol = null,
  skipVisitingNodeSymbol = false,
  exposeEdits = false
} = {}) => {
  const skipSymbol = Symbol('skip');
  const skipping = new Array(visitors.length).fill(skipSymbol);
  return {
    enter(node, key, parent, path, ancestors, link) {
      let currentNode = node;
      let hasChanged = false;
      const linkProxy = {
        ...link,
        replaceWith(newNode, replacer) {
          link.replaceWith(newNode, replacer);
          currentNode = newNode;
        }
      };
      for (let i = 0; i < visitors.length; i += 1) {
        if (skipping[i] === skipSymbol) {
          const visitFn = visitFnGetter(visitors[i], nodeTypeGetter(currentNode), false);
          if (typeof visitFn === 'function') {
            const result = visitFn.call(visitors[i], currentNode, key, parent, path, ancestors, linkProxy);

            // check if the visitor is async
            if (typeof (result === null || result === void 0 ? void 0 : result.then) === 'function') {
              throw new ApiDOMStructuredError('Async visitor not supported in sync mode', {
                visitor: visitors[i],
                visitFn
              });
            }
            if (result === skipVisitingNodeSymbol) {
              skipping[i] = currentNode;
            } else if (result === breakSymbol) {
              skipping[i] = breakSymbol;
            } else if (result === deleteNodeSymbol) {
              return result;
            } else if (result !== undefined) {
              if (exposeEdits) {
                currentNode = result;
                hasChanged = true;
              } else {
                return result;
              }
            }
          }
        }
      }
      return hasChanged ? currentNode : undefined;
    },
    leave(node, key, parent, path, ancestors, link) {
      let currentNode = node;
      const linkProxy = {
        ...link,
        replaceWith(newNode, replacer) {
          link.replaceWith(newNode, replacer);
          currentNode = newNode;
        }
      };
      for (let i = 0; i < visitors.length; i += 1) {
        if (skipping[i] === skipSymbol) {
          const visitFn = visitFnGetter(visitors[i], nodeTypeGetter(currentNode), true);
          if (typeof visitFn === 'function') {
            const result = visitFn.call(visitors[i], currentNode, key, parent, path, ancestors, linkProxy);

            // check if the visitor is async
            if (typeof (result === null || result === void 0 ? void 0 : result.then) === 'function') {
              throw new ApiDOMStructuredError('Async visitor not supported in sync mode', {
                visitor: visitors[i],
                visitFn
              });
            }
            if (result === breakSymbol) {
              skipping[i] = breakSymbol;
            } else if (result !== undefined && result !== skipVisitingNodeSymbol) {
              return result;
            }
          }
        } else if (skipping[i] === currentNode) {
          skipping[i] = skipSymbol;
        }
      }
      return undefined;
    }
  };
};
const mergeAllAsync = (visitors, {
  visitFnGetter = getVisitFn,
  nodeTypeGetter = getNodeType,
  breakSymbol = BREAK,
  deleteNodeSymbol = null,
  skipVisitingNodeSymbol = false,
  exposeEdits = false
} = {}) => {
  const skipSymbol = Symbol('skip');
  const skipping = new Array(visitors.length).fill(skipSymbol);
  return {
    async enter(node, key, parent, path, ancestors, link) {
      let currentNode = node;
      let hasChanged = false;
      const linkProxy = {
        ...link,
        replaceWith(newNode, replacer) {
          link.replaceWith(newNode, replacer);
          currentNode = newNode;
        }
      };
      for (let i = 0; i < visitors.length; i += 1) {
        if (skipping[i] === skipSymbol) {
          const visitFn = visitFnGetter(visitors[i], nodeTypeGetter(currentNode), false);
          if (typeof visitFn === 'function') {
            // eslint-disable-next-line no-await-in-loop
            const result = await visitFn.call(visitors[i], currentNode, key, parent, path, ancestors, linkProxy);
            if (result === skipVisitingNodeSymbol) {
              skipping[i] = currentNode;
            } else if (result === breakSymbol) {
              skipping[i] = breakSymbol;
            } else if (result === deleteNodeSymbol) {
              return result;
            } else if (result !== undefined) {
              if (exposeEdits) {
                currentNode = result;
                hasChanged = true;
              } else {
                return result;
              }
            }
          }
        }
      }
      return hasChanged ? currentNode : undefined;
    },
    async leave(node, key, parent, path, ancestors, link) {
      let currentNode = node;
      const linkProxy = {
        ...link,
        replaceWith(newNode, replacer) {
          link.replaceWith(newNode, replacer);
          currentNode = newNode;
        }
      };
      for (let i = 0; i < visitors.length; i += 1) {
        if (skipping[i] === skipSymbol) {
          const visitFn = visitFnGetter(visitors[i], nodeTypeGetter(currentNode), true);
          if (typeof visitFn === 'function') {
            // eslint-disable-next-line no-await-in-loop
            const result = await visitFn.call(visitors[i], currentNode, key, parent, path, ancestors, linkProxy);
            if (result === breakSymbol) {
              skipping[i] = breakSymbol;
            } else if (result !== undefined && result !== skipVisitingNodeSymbol) {
              return result;
            }
          }
        } else if (skipping[i] === currentNode) {
          skipping[i] = skipSymbol;
        }
      }
      return undefined;
    }
  };
};
mergeAll[Symbol.for('nodejs.util.promisify.custom')] = mergeAllAsync;

/* eslint-disable no-continue, no-param-reassign */
/**
 * visit() will walk through an AST using a preorder depth first traversal, calling
 * the visitor's enter function at each node in the traversal, and calling the
 * leave function after visiting that node and all of its child nodes.
 *
 * By returning different values from the enter and leave functions, the
 * behavior of the visitor can be altered, including skipping over a sub-tree of
 * the AST (by returning false), editing the AST by returning a value or null
 * to remove the value, or to stop the whole traversal by returning BREAK.
 *
 * When using visit() to edit an AST, the original AST will not be modified, and
 * a new version of the AST with the changes applied will be returned from the
 * visit function.
 *
 * @example
 * ```
 *     const editedAST = visit(ast, {
 *       enter(node, key, parent, path, ancestors) {
 *         // return
 *         //   undefined: no action
 *         //   false: skip visiting this node
 *         //   BREAK: stop visiting altogether
 *         //   null: delete this node
 *         //   any value: replace this node with the returned value
 *       },
 *       leave(node, key, parent, path, ancestors) {
 *         // return
 *         //   undefined: no action
 *         //   false: no action
 *         //   BREAK: stop visiting altogether
 *         //   null: delete this node
 *         //   any value: replace this node with the returned value
 *       }
 *     });
 *```
 * Alternatively to providing enter() and leave() functions, a visitor can
 * instead provide functions named the same as the kinds of AST nodes, or
 * enter/leave visitors at a named key, leading to four permutations of
 * visitor API:
 *
 * 1) Named visitors triggered when entering a node a specific kind.
 * ```
 *     visit(ast, {
 *       Kind(node) {
 *         // enter the "Kind" node
 *       }
 *     })
 * ```
 * 2) Named visitors that trigger upon entering and leaving a node of
 *    a specific kind.
 * ```
 *     visit(ast, {
 *       Kind: {
 *         enter(node) {
 *           // enter the "Kind" node
 *         }
 *         leave(node) {
 *           // leave the "Kind" node
 *         }
 *       }
 *     })
 * ```
 * 3) Generic visitors that trigger upon entering and leaving any node.
 * ```
 *     visit(ast, {
 *       enter(node) {
 *         // enter any node
 *       },
 *       leave(node) {
 *         // leave any node
 *       }
 *     })
 * ```
 * 4) Parallel visitors for entering and leaving nodes of a specific kind.
 * ```
 *     visit(ast, {
 *       enter: {
 *         Kind(node) {
 *           // enter the "Kind" node
 *         }
 *       },
 *       leave: {
 *         Kind(node) {
 *           // leave the "Kind" node
 *         }
 *       }
 *     })
 * ```
 *  sig `visit :: (Node, Visitor, Options)`
 *
 *  sig `Options = { keyMap: Object, state: Object }`
 *
 *  @public
 */
export const visit = (
// @ts-ignore
root,
// @ts-ignore
visitor, {
  keyMap = null,
  state = {},
  breakSymbol = BREAK,
  deleteNodeSymbol = null,
  skipVisitingNodeSymbol = false,
  visitFnGetter = getVisitFn,
  nodeTypeGetter = getNodeType,
  nodePredicate = isNode,
  nodeCloneFn = cloneNode,
  detectCycles = true,
  detectCyclesCallback = null
} = {}) => {
  const visitorKeys = keyMap || {};
  let stack;
  let inArray = Array.isArray(root);
  let keys = [root];
  let index = -1;
  let parent;
  let edits = [];
  let node = root;
  const path = [];
  // @ts-ignore
  const ancestors = [];
  do {
    index += 1;
    const isLeaving = index === keys.length;
    let key;
    const isEdited = isLeaving && edits.length !== 0;
    if (isLeaving) {
      key = ancestors.length === 0 ? undefined : path.pop();
      node = parent;
      // @ts-ignore
      parent = ancestors.pop();
      if (isEdited) {
        if (inArray) {
          // @ts-ignore; creating clone
          node = node.slice();
          let editOffset = 0;
          for (const [editKey, editValue] of edits) {
            const arrayKey = editKey - editOffset;
            if (editValue === deleteNodeSymbol) {
              node.splice(arrayKey, 1);
              editOffset += 1;
            } else {
              node[arrayKey] = editValue;
            }
          }
        } else {
          // creating clone
          node = nodeCloneFn(node);
          for (const [editKey, editValue] of edits) {
            node[editKey] = editValue;
          }
        }
      }
      index = stack.index;
      keys = stack.keys;
      // @ts-ignore
      edits = stack.edits;
      // @ts-ignore
      inArray = stack.inArray;
      // @ts-ignore
      stack = stack.prev;
    } else if (parent !== deleteNodeSymbol && parent !== undefined) {
      key = inArray ? index : keys[index];
      node = parent[key];
      if (node === deleteNodeSymbol || node === undefined) {
        continue;
      }
      path.push(key);
    }
    let result;
    if (!Array.isArray(node)) {
      var _result;
      if (!nodePredicate(node)) {
        throw new ApiDOMStructuredError(`Invalid AST Node:  ${String(node)}`, {
          node
        });
      }

      // cycle detected; skipping over a sub-tree to avoid recursion
      if (detectCycles && ancestors.includes(node)) {
        if (typeof detectCyclesCallback === 'function') {
          // @ts-ignore
          detectCyclesCallback(node, key, parent, path, ancestors);
        }
        path.pop();
        continue;
      }
      // call appropriate visitor function if available
      const visitFn = visitFnGetter(visitor, nodeTypeGetter(node), isLeaving);
      if (visitFn) {
        // assign state
        for (const [stateKey, stateValue] of Object.entries(state)) {
          visitor[stateKey] = stateValue;
        }
        const link = {
          // eslint-disable-next-line @typescript-eslint/no-loop-func
          replaceWith(newNode, replacer) {
            if (typeof replacer === 'function') {
              replacer(newNode, node, key, parent, path, ancestors);
            } else if (parent) {
              parent[key] = newNode;
            }
            if (!isLeaving) {
              node = newNode;
            }
          }
        };

        // retrieve result
        result = visitFn.call(visitor, node, key, parent, path, ancestors, link);
      }

      // check if the visitor is async
      if (typeof ((_result = result) === null || _result === void 0 ? void 0 : _result.then) === 'function') {
        throw new ApiDOMStructuredError('Async visitor not supported in sync mode', {
          visitor,
          visitFn
        });
      }
      if (result === breakSymbol) {
        break;
      }
      if (result === skipVisitingNodeSymbol) {
        if (!isLeaving) {
          path.pop();
          continue;
        }
      } else if (result !== undefined) {
        edits.push([key, result]);
        if (!isLeaving) {
          if (nodePredicate(result)) {
            node = result;
          } else {
            path.pop();
            continue;
          }
        }
      }
    }
    if (result === undefined && isEdited) {
      edits.push([key, node]);
    }
    if (!isLeaving) {
      var _visitorKeys$nodeType;
      stack = {
        inArray,
        index,
        keys,
        edits,
        prev: stack
      };
      inArray = Array.isArray(node);
      // @ts-ignore
      keys = inArray ? node : (_visitorKeys$nodeType = visitorKeys[nodeTypeGetter(node)]) !== null && _visitorKeys$nodeType !== void 0 ? _visitorKeys$nodeType : [];
      index = -1;
      edits = [];
      if (parent !== deleteNodeSymbol && parent !== undefined) {
        ancestors.push(parent);
      }
      parent = node;
    }
  } while (stack !== undefined);
  if (edits.length !== 0) {
    return edits[edits.length - 1][1]; // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.at in future
  }
  return root;
};

/**
 * Asynchronous version of visit.
 */
// @ts-ignore
visit[Symbol.for('nodejs.util.promisify.custom')] = async (
// @ts-ignore
root,
// @ts-ignore
visitor, {
  keyMap = null,
  state = {},
  breakSymbol = BREAK,
  deleteNodeSymbol = null,
  skipVisitingNodeSymbol = false,
  visitFnGetter = getVisitFn,
  nodeTypeGetter = getNodeType,
  nodePredicate = isNode,
  nodeCloneFn = cloneNode,
  detectCycles = true,
  detectCyclesCallback = null
} = {}) => {
  const visitorKeys = keyMap || {};
  let stack;
  let inArray = Array.isArray(root);
  let keys = [root];
  let index = -1;
  let parent;
  let edits = [];
  let node = root;
  const path = [];
  // @ts-ignore
  const ancestors = [];
  do {
    index += 1;
    const isLeaving = index === keys.length;
    let key;
    const isEdited = isLeaving && edits.length !== 0;
    if (isLeaving) {
      key = ancestors.length === 0 ? undefined : path.pop();
      node = parent;
      // @ts-ignore
      parent = ancestors.pop();
      if (isEdited) {
        if (inArray) {
          // @ts-ignore; creating clone
          node = node.slice();
          let editOffset = 0;
          for (const [editKey, editValue] of edits) {
            const arrayKey = editKey - editOffset;
            if (editValue === deleteNodeSymbol) {
              node.splice(arrayKey, 1);
              editOffset += 1;
            } else {
              node[arrayKey] = editValue;
            }
          }
        } else {
          // creating clone
          node = nodeCloneFn(node);
          for (const [editKey, editValue] of edits) {
            node[editKey] = editValue;
          }
        }
      }
      index = stack.index;
      keys = stack.keys;
      // @ts-ignore
      edits = stack.edits;
      // @ts-ignore
      inArray = stack.inArray;
      // @ts-ignore
      stack = stack.prev;
    } else if (parent !== deleteNodeSymbol && parent !== undefined) {
      key = inArray ? index : keys[index];
      node = parent[key];
      if (node === deleteNodeSymbol || node === undefined) {
        continue;
      }
      path.push(key);
    }
    let result;
    if (!Array.isArray(node)) {
      if (!nodePredicate(node)) {
        throw new ApiDOMStructuredError(`Invalid AST Node: ${String(node)}`, {
          node
        });
      }

      // cycle detected; skipping over a sub-tree to avoid recursion
      if (detectCycles && ancestors.includes(node)) {
        if (typeof detectCyclesCallback === 'function') {
          // @ts-ignore
          detectCyclesCallback(node, key, parent, path, ancestors);
        }
        path.pop();
        continue;
      }
      const visitFn = visitFnGetter(visitor, nodeTypeGetter(node), isLeaving);
      if (visitFn) {
        // assign state
        for (const [stateKey, stateValue] of Object.entries(state)) {
          visitor[stateKey] = stateValue;
        }
        const link = {
          // eslint-disable-next-line @typescript-eslint/no-loop-func
          replaceWith(newNode, replacer) {
            if (typeof replacer === 'function') {
              replacer(newNode, node, key, parent, path, ancestors);
            } else if (parent) {
              parent[key] = newNode;
            }
            if (!isLeaving) {
              node = newNode;
            }
          }
        };

        // retrieve result
        result = await visitFn.call(visitor, node, key, parent, path, ancestors, link); // eslint-disable-line no-await-in-loop
      }
      if (result === breakSymbol) {
        break;
      }
      if (result === skipVisitingNodeSymbol) {
        if (!isLeaving) {
          path.pop();
          continue;
        }
      } else if (result !== undefined) {
        edits.push([key, result]);
        if (!isLeaving) {
          if (nodePredicate(result)) {
            node = result;
          } else {
            path.pop();
            continue;
          }
        }
      }
    }
    if (result === undefined && isEdited) {
      edits.push([key, node]);
    }
    if (!isLeaving) {
      var _visitorKeys$nodeType2;
      stack = {
        inArray,
        index,
        keys,
        edits,
        prev: stack
      };
      inArray = Array.isArray(node);
      // @ts-ignore
      keys = inArray ? node : (_visitorKeys$nodeType2 = visitorKeys[nodeTypeGetter(node)]) !== null && _visitorKeys$nodeType2 !== void 0 ? _visitorKeys$nodeType2 : [];
      index = -1;
      edits = [];
      if (parent !== deleteNodeSymbol && parent !== undefined) {
        ancestors.push(parent);
      }
      parent = node;
    }
  } while (stack !== undefined);
  if (edits.length !== 0) {
    return edits[edits.length - 1][1]; // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.at in future
  }
  return root;
};

/* eslint-enable */