"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _NormalizeStorage = _interopRequireDefault(require("./NormalizeStorage.cjs"));
/**
 * Override of Schema.example and Schema.examples field inside the Header Objects.
 *
 * Header Object has two fixed fields:
 *  - `example` of type `Any`
 *  - `examples` of type `Map[string, Example Object | Reference Object]`
 *
 * OpenAPI 3.1 specification excerpt that defines the override behavior:
 *
 * The example value SHALL override the example provided by the schema.
 * Furthermore, if referencing a schema that contains an example, the examples value SHALL override the example provided by the schema.
 *
 * NOTE: this plugin is idempotent
 * @public
 */

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
  let storage;
  return {
    visitor: {
      OpenApi3_1Element: {
        enter(element) {
          storage = new _NormalizeStorage.default(element, storageField, 'header-examples');
        },
        leave() {
          storage = undefined;
        }
      },
      HeaderElement: {
        leave(headerElement, key, parent, path, ancestors) {
          var _headerElement$schema, _headerElement$schema2;
          // skip visiting this Header Object
          if (ancestors.some(predicates.isComponentsElement)) {
            return;
          }

          // no Header.schema field present
          if (typeof headerElement.schema === 'undefined' || !predicates.isSchemaElement(headerElement.schema)) {
            return;
          }
          // Schema contains no example
          if (typeof ((_headerElement$schema = headerElement.schema) == null ? void 0 : _headerElement$schema.example) === 'undefined' && typeof ((_headerElement$schema2 = headerElement.schema) == null ? void 0 : _headerElement$schema2.examples) === 'undefined') {
            return;
          }
          const headerJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, headerElement]);

          // skip visiting this Header Object if it's already normalized
          if (storage.includes(headerJSONPointer)) {
            return;
          }

          /**
           * Header.examples and Schema.examples have preferences over the older
           * and deprected `example` field.
           */
          if (typeof headerElement.examples !== 'undefined' && predicates.isObjectElement(headerElement.examples)) {
            // @ts-ignore
            const examples = headerElement.examples.map(example => {
              return _apidomCore.cloneDeep.safe(example.value);
            });
            if (typeof headerElement.schema.examples !== 'undefined') {
              headerElement.schema.set('examples', examples);
              storage.append(headerJSONPointer);
            }
            if (typeof headerElement.schema.example !== 'undefined') {
              headerElement.schema.set('example', examples[0]);
              storage.append(headerJSONPointer);
            }
            return;
          }

          /**
           * Handle deprecated `example` field.
           */
          if (typeof headerElement.example !== 'undefined') {
            if (typeof headerElement.schema.examples !== 'undefined') {
              headerElement.schema.set('examples', [(0, _apidomCore.cloneDeep)(headerElement.example)]);
              storage.append(headerJSONPointer);
            }
            if (typeof headerElement.schema.example !== 'undefined') {
              headerElement.schema.set('example', (0, _apidomCore.cloneDeep)(headerElement.example));
              storage.append(headerJSONPointer);
            }
          }
        }
      }
    }
  };
};
var _default = exports.default = plugin;