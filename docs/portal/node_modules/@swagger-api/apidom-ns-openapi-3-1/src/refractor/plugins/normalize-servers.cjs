"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _NormalizeStorage = _interopRequireDefault(require("./normalize-header-examples/NormalizeStorage.cjs"));
/**
 * Override of Server Objects.
 *
 * List of Server Objects can be defined in OpenAPI 3.1 on multiple levels:
 *
 *  - OpenAPI.servers
 *  - PathItem.servers
 *  - Operation.servers
 *
 * If a servers array is specified at the OpenAPI Object level, it will be overridden by `PathItem`.servers.
 * If a servers array is specified at the Path Item Object or OpenAPI Object level, it will be overridden by Operation.servers.
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
    namespace,
    ancestorLineageToJSONPointer,
    predicates
  } = toolbox;
  let storage;
  return {
    visitor: {
      OpenApi3_1Element: {
        enter(openapiElement) {
          const isServersUndefined = typeof openapiElement.servers === 'undefined';
          const isServersArrayElement = predicates.isArrayElement(openapiElement.servers);
          const isServersEmpty = isServersArrayElement && openapiElement.servers.length === 0;
          // @ts-ignore
          const defaultServer = namespace.elements.Server.refract({
            url: '/'
          });
          if (isServersUndefined || !isServersArrayElement) {
            openapiElement.servers = new _apidomNsOpenapi.ServersElement([defaultServer]);
          } else if (isServersArrayElement && isServersEmpty) {
            openapiElement.servers.push(defaultServer);
          }
          storage = new _NormalizeStorage.default(openapiElement, storageField, 'servers');
        },
        leave() {
          storage = undefined;
        }
      },
      PathItemElement(pathItemElement, key, parent, path, ancestors) {
        // skip visiting this Path Item
        if (ancestors.some(predicates.isComponentsElement)) return;
        if (!ancestors.some(predicates.isOpenApi3_1Element)) return;
        const pathItemJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, pathItemElement]);

        // skip visiting this Path Item Object if it's already normalized
        if (storage.includes(pathItemJSONPointer)) {
          return;
        }
        const parentOpenapiElement = ancestors.find(predicates.isOpenApi3_1Element);
        const isServersUndefined = typeof pathItemElement.servers === 'undefined';
        const isServersArrayElement = predicates.isArrayElement(pathItemElement.servers);
        const isServersEmpty = isServersArrayElement && pathItemElement.servers.length === 0;

        // duplicate OpenAPI.servers into this Path Item object
        if (predicates.isOpenApi3_1Element(parentOpenapiElement)) {
          var _parentOpenapiElement;
          const openapiServersContent = (_parentOpenapiElement = parentOpenapiElement.servers) == null ? void 0 : _parentOpenapiElement.content;
          const openapiServers = openapiServersContent != null ? openapiServersContent : [];
          if (isServersUndefined || !isServersArrayElement) {
            pathItemElement.servers = new _apidomNsOpenapi.PathItemServersElement(openapiServers);
          } else if (isServersArrayElement && isServersEmpty) {
            openapiServers.forEach(server => {
              pathItemElement.servers.push(server);
            });
          }
          storage.append(pathItemJSONPointer);
        }
      },
      OperationElement(operationElement, key, parent, path, ancestors) {
        // skip visiting this Operation
        if (ancestors.some(predicates.isComponentsElement)) return;
        if (!ancestors.some(predicates.isOpenApi3_1Element)) return;
        const operationJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, operationElement]);

        // skip visiting this Operation Object if it's already normalized
        if (storage.includes(operationJSONPointer)) {
          return;
        }

        // @TODO(vladimir.gorej@gmail.com): can be replaced by Array.prototype.findLast in future
        const parentPathItemElement = [...ancestors].reverse().find(predicates.isPathItemElement);
        const isServersUndefined = typeof operationElement.servers === 'undefined';
        const isServersArrayElement = predicates.isArrayElement(operationElement.servers);
        const isServersEmpty = isServersArrayElement && operationElement.servers.length === 0;
        if (predicates.isPathItemElement(parentPathItemElement)) {
          var _parentPathItemElemen;
          const pathItemServersContent = (_parentPathItemElemen = parentPathItemElement.servers) == null ? void 0 : _parentPathItemElemen.content;
          const pathItemServers = pathItemServersContent != null ? pathItemServersContent : [];
          if (isServersUndefined || !isServersArrayElement) {
            // duplicate parent PathItem.servers into this Operation object
            operationElement.servers = new _apidomNsOpenapi.OperationServersElement(pathItemServers);
          } else if (isServersArrayElement && isServersEmpty) {
            pathItemServers.forEach(server => {
              operationElement.servers.push(server);
            });
          }
          storage.append(operationJSONPointer);
        }
      }
    }
  };
};
/* eslint-enable */
var _default = exports.default = plugin;