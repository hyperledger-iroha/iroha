"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _NormalizeStorage = _interopRequireDefault(require("./normalize-header-examples/NormalizeStorage.cjs"));
/**
 * Inheritance of Parameter Objects.
 *
 * OpenAPI 3.1 specification excerpt that defines the inheritance behavior:
 *
 * A list of parameters that are applicable for this operation. If a parameter is already defined at the Path Item,
 * the new definition will override it but can never remove it. The list MUST NOT include duplicated parameters.
 * A unique parameter is defined by a combination of a name and location.
 *
 * NOTE: this plugin is idempotent
 * @public
 */

/* eslint-disable no-param-reassign */
/**
 * @public
 */
const plugin = ({
  storageField = 'x-normalized'
} = {}) => toolbox => {
  const {
    predicates,
    ancestorLineageToJSONPointer
  } = toolbox;
  /**
   * Establishes identity between two Parameter Objects.
   *
   * {@link https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md#user-content-operationparameters}
   */
  const parameterEquals = (parameter1, parameter2) => {
    if (!predicates.isParameterElement(parameter1)) return false;
    if (!predicates.isParameterElement(parameter2)) return false;
    if (!predicates.isStringElement(parameter1.name)) return false;
    if (!predicates.isStringElement(parameter1.in)) return false;
    if (!predicates.isStringElement(parameter2.name)) return false;
    if (!predicates.isStringElement(parameter2.in)) return false;
    return (0, _apidomCore.toValue)(parameter1.name) === (0, _apidomCore.toValue)(parameter2.name) && (0, _apidomCore.toValue)(parameter1.in) === (0, _apidomCore.toValue)(parameter2.in);
  };
  const pathItemParameters = [];
  let storage;
  return {
    visitor: {
      OpenApi3_1Element: {
        enter(element) {
          storage = new _NormalizeStorage.default(element, storageField, 'parameters');
        },
        leave() {
          storage = undefined;
        }
      },
      PathItemElement: {
        enter(pathItemElement, key, parent, path, ancestors) {
          // skip visiting this Path Item
          if (ancestors.some(predicates.isComponentsElement)) {
            return;
          }
          const {
            parameters
          } = pathItemElement;
          if (predicates.isArrayElement(parameters)) {
            pathItemParameters.push([...parameters.content]);
          } else {
            pathItemParameters.push([]);
          }
        },
        leave() {
          pathItemParameters.pop();
        }
      },
      OperationElement: {
        leave(operationElement, key, parent, path, ancestors) {
          const parentPathItemParameters = (0, _ramda.last)(pathItemParameters);

          // no Path Item Object parameters to inherit from
          if (!Array.isArray(parentPathItemParameters) || parentPathItemParameters.length === 0) {
            return;
          }
          const operationJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, operationElement]);

          // skip visiting this Operation Object if it's already normalized
          if (storage.includes(operationJSONPointer)) {
            return;
          }
          const operationParameters = (0, _ramda.pathOr)([], ['parameters', 'content'], operationElement);

          // prefers the first item if two items compare equal based on the predicate
          const mergedParameters = (0, _ramda.uniqWith)(parameterEquals, [...operationParameters, ...parentPathItemParameters]);
          operationElement.parameters = new _apidomNsOpenapi.OperationParametersElement(mergedParameters);
          storage.append(operationJSONPointer);
        }
      }
    }
  };
};
/* eslint-enable */
var _default = exports.default = plugin;