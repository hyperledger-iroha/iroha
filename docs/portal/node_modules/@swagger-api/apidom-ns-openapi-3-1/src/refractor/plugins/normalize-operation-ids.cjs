"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _NormalizeStorage = _interopRequireDefault(require("./normalize-header-examples/NormalizeStorage.cjs"));
const removeSpaces = operationId => {
  return operationId.replace(/\s/g, '');
};
const replaceSpecialCharsWithUnderscore = operationId => {
  return operationId.replace(/\W/gi, '_');
};
const createNormalizedOperationId = (path, method) => {
  const normalizedMethod = replaceSpecialCharsWithUnderscore(removeSpaces(method.toLowerCase()));
  const normalizedPath = replaceSpecialCharsWithUnderscore(removeSpaces(path));
  return `${normalizedMethod}${normalizedPath}`;
};
const normalizeOperationId = (operationId, path, method) => {
  const withoutSpaces = removeSpaces(operationId);
  if (withoutSpaces.length > 0) {
    return replaceSpecialCharsWithUnderscore(withoutSpaces);
  }
  return createNormalizedOperationId(path, method);
};

/**
 * Normalization of Operation.operationId field.
 *
 * This normalization is not guided by OpenAPI 3.1 specification.
 *
 * Existing Operation.operationId fields are normalized into snake case form.
 *
 * Operation Objects, that do not define operationId field, are left untouched.
 *
 * Original operationId is stored in meta and as new `__originalOperationId` field.
 *
 * This plugin also guarantees the uniqueness of all defined Operation.operationId fields,
 * and make sure Link.operationId fields are pointing to correct and normalized Operation.operationId fields.
 *
 * NOTE: this plugin is idempotent
 * @public
 */

/* eslint-disable no-param-reassign */
/**
 * @public
 */
const plugin = ({
  storageField = 'x-normalized',
  operationIdNormalizer = normalizeOperationId
} = {}) => toolbox => {
  const {
    predicates,
    ancestorLineageToJSONPointer,
    namespace
  } = toolbox;
  const pathTemplates = [];
  const normalizedOperations = [];
  const links = [];
  let storage;
  return {
    visitor: {
      OpenApi3_1Element: {
        enter(element) {
          storage = new _NormalizeStorage.default(element, storageField, 'operation-ids');
        },
        leave() {
          // group normalized operations by normalized operationId
          const normalizedOperationGroups = (0, _ramda.groupBy)(operationElement => {
            return (0, _apidomCore.toValue)(operationElement.operationId);
          }, normalizedOperations);

          // append incremental numerical suffixes to identical operationIds
          Object.entries(normalizedOperationGroups).forEach(([normalizedOperationId, operationElements]) => {
            if (!Array.isArray(operationElements)) return;
            if (operationElements.length <= 1) return;
            operationElements.forEach((operationElement, index) => {
              const indexedNormalizedOperationId = `${normalizedOperationId}${index + 1}`;
              // @ts-ignore
              operationElement.operationId = new namespace.elements.String(indexedNormalizedOperationId);
            });
          });

          // rectify possibly broken Link.operationId fields
          links.forEach(linkElement => {
            if (typeof linkElement.operationId === 'undefined') return;
            const linkOperationId = String((0, _apidomCore.toValue)(linkElement.operationId));
            const operationElement = normalizedOperations.find(normalizedOperationElement => {
              const originalOperationId = (0, _apidomCore.toValue)(normalizedOperationElement.meta.get('originalOperationId'));
              return originalOperationId === linkOperationId;
            });

            // Link Object doesn't need to be rectified
            if (typeof operationElement === 'undefined') return;
            linkElement.operationId = _apidomCore.cloneDeep.safe(operationElement.operationId);
            linkElement.meta.set('originalOperationId', linkOperationId);
            linkElement.set('__originalOperationId', linkOperationId);
          });

          // cleanup the references
          normalizedOperations.length = 0;
          links.length = 0;
          storage = undefined;
        }
      },
      PathItemElement: {
        enter(pathItemElement) {
          // `path` meta may not be always available, e.g. in Callback Object or Components Object
          const pathTemplate = (0, _ramda.defaultTo)('path', (0, _apidomCore.toValue)(pathItemElement.meta.get('path')));
          pathTemplates.push(pathTemplate);
        },
        leave() {
          pathTemplates.pop();
        }
      },
      OperationElement: {
        enter(operationElement, key, parent, path, ancestors) {
          // operationId field is undefined, needs no normalization
          if (typeof operationElement.operationId === 'undefined') return;
          const operationJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, operationElement]);

          // skip visiting this Operation Object if it's already normalized
          if (storage.includes(operationJSONPointer)) {
            return;
          }

          // cast operationId to string type
          const originalOperationId = String((0, _apidomCore.toValue)(operationElement.operationId));
          // perform operationId normalization
          const pathTemplate = (0, _ramda.last)(pathTemplates);
          // `http-method` meta may not be always available, e.g. in Callback Object or Components Object
          const method = (0, _ramda.defaultTo)('method', (0, _apidomCore.toValue)(operationElement.meta.get('http-method')));
          const normalizedOperationId = operationIdNormalizer(originalOperationId, pathTemplate, method);

          // normalization is not necessary
          if (originalOperationId === normalizedOperationId) return;

          // @ts-ignore
          operationElement.operationId = new namespace.elements.String(normalizedOperationId);
          operationElement.set('__originalOperationId', originalOperationId);
          operationElement.meta.set('originalOperationId', originalOperationId);
          normalizedOperations.push(operationElement);
          storage.append(operationJSONPointer);
        }
      },
      LinkElement: {
        leave(linkElement) {
          // make sure this Link elements doesn't come from base namespace
          if (!predicates.isLinkElement(linkElement)) return;
          // ignore Link Objects with undefined operationId
          if (typeof linkElement.operationId === 'undefined') return;
          links.push(linkElement);
        }
      }
    }
  };
};
/* eslint-enable */
var _default = exports.default = plugin;