"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _NormalizeStorage = _interopRequireDefault(require("./normalize-header-examples/NormalizeStorage.cjs"));
/**
 * Override of Security Requirement Objects.
 *
 * OpenAPI 3.1 specification excerpt that defines the override behavior:
 *
 * Operation.security definition overrides any declared top-level security.
 * To remove a top-level security declaration, an empty array can be used.
 * When a list of Security Requirement Objects is defined on the OpenAPI Object or Operation Object,
 * only one of the Security Requirement Objects in the list needs to be satisfied to authorize the request.
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
  let topLevelSecurity;
  let storage;
  return {
    visitor: {
      OpenApi3_1Element: {
        enter(openapiElement) {
          storage = new _NormalizeStorage.default(openapiElement, storageField, 'security-requirements');
          if (predicates.isArrayElement(openapiElement.security)) {
            topLevelSecurity = openapiElement.security;
          }
        },
        leave() {
          storage = undefined;
          topLevelSecurity = undefined;
        }
      },
      OperationElement: {
        leave(operationElement, key, parent, path, ancestors) {
          // skip visiting this Operation
          if (ancestors.some(predicates.isComponentsElement)) {
            return;
          }
          const operationJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, operationElement]);

          // skip visiting this Operation Object if it's already normalized
          if (storage.includes(operationJSONPointer)) {
            return;
          }
          const missingOperationLevelSecurity = typeof operationElement.security === 'undefined';
          const hasTopLevelSecurity = typeof topLevelSecurity !== 'undefined';
          if (missingOperationLevelSecurity && hasTopLevelSecurity) {
            var _topLevelSecurity;
            operationElement.security = new _apidomNsOpenapi.OperationSecurityElement((_topLevelSecurity = topLevelSecurity) == null ? void 0 : _topLevelSecurity.content);
            storage.append(operationJSONPointer);
          }
        }
      }
    }
  };
};
/* eslint-enable */
var _default = exports.default = plugin;