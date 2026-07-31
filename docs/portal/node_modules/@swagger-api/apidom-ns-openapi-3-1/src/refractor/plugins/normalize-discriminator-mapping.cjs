"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _NormalizeStorage = _interopRequireDefault(require("./normalize-header-examples/NormalizeStorage.cjs"));
var _predicates = require("../../predicates.cjs");
var _Discriminator = _interopRequireDefault(require("../../elements/Discriminator.cjs"));
/**
 * Normalization of Discriminator.mapping field.
 *
 * Discriminator.mapping fields are normalized by adding missing mappings from oneOf/anyOf items
 * of the parent Schema Object and transforming existing mappings to Schema Objects.
 *
 * In case of allOf discriminator, the plugin will add missing mappings based on
 * allOf items of other Schema Objects.
 *
 * The normalized mapping is stored in the Schema.discriminator field as `x-normalized-mapping`.
 *
 * This plugin is designed to be used on dereferenced OpenAPI 3.1 documents.
 *
 * NOTE: this plugin is idempotent
 * @public
 */

/**
 * @public
 */
const plugin = ({
  storageField = 'x-normalized',
  baseURI = ''
} = {}) => toolbox => {
  const {
    ancestorLineageToJSONPointer
  } = toolbox;
  let storage;
  let allOfDiscriminatorMapping;
  return {
    visitor: {
      OpenApi3_1Element: {
        enter(element) {
          var _element$getMetaPrope;
          storage = new _NormalizeStorage.default(element, storageField, 'discriminator-mapping');
          allOfDiscriminatorMapping = (_element$getMetaPrope = element.getMetaProperty('allOfDiscriminatorMapping')) != null ? _element$getMetaPrope : new _apidomCore.ObjectElement();
        },
        leave() {
          storage = undefined;
        }
      },
      SchemaElement: {
        leave(schemaElement, key, parent, path, ancestors) {
          var _parentElement$classe, _schemaElement$discri;
          // no Schema.discriminator field present
          if (!(0, _apidomNsOpenapi.isDiscriminatorElement)(schemaElement.discriminator)) {
            return;
          }
          const schemaJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, schemaElement]);

          // skip visiting this Schema Object if it's already normalized
          if (storage.includes(schemaJSONPointer)) {
            return;
          }

          // skip if both oneOf and anyOf are present
          if ((0, _apidomCore.isArrayElement)(schemaElement.oneOf) && (0, _apidomCore.isArrayElement)(schemaElement.anyOf)) {
            return;
          }
          const parentElement = ancestors[ancestors.length - 1];
          const schemaName = schemaElement.getMetaProperty('schemaName');
          const allOfMapping = allOfDiscriminatorMapping.getMember((0, _apidomCore.toValue)(schemaName));
          const hasAllOfMapping =
          // @ts-ignore
          allOfMapping && !(parentElement != null && (_parentElement$classe = parentElement.classes) != null && _parentElement$classe.contains('json-schema-allOf'));

          // skip if neither oneOf, anyOf nor allOf is present
          if (!(0, _apidomCore.isArrayElement)(schemaElement.oneOf) && !(0, _apidomCore.isArrayElement)(schemaElement.anyOf) && !hasAllOfMapping) {
            return;
          }
          const mapping = (_schemaElement$discri = schemaElement.discriminator.get('mapping')) != null ? _schemaElement$discri : new _apidomCore.ObjectElement();
          const normalizedMapping = new _apidomCore.ObjectElement();
          let isNormalized = true;
          const items = (0, _apidomCore.isArrayElement)(schemaElement.oneOf) ? schemaElement.oneOf : (0, _apidomCore.isArrayElement)(schemaElement.anyOf) ? schemaElement.anyOf : allOfMapping.value;
          items.forEach(item => {
            if (!(0, _predicates.isSchemaElement)(item)) {
              return;
            }
            if ((0, _apidomNsOpenapi.isReferenceLikeElement)(item)) {
              isNormalized = false;
              return;
            }
            const metaRefFields = (0, _apidomCore.toValue)(item.getMetaProperty('ref-fields'));
            const metaRefOrigin = (0, _apidomCore.toValue)(item.getMetaProperty('ref-origin'));
            const metaSchemaName = (0, _apidomCore.toValue)(item.getMetaProperty('schemaName'));

            /**
             * handle external references and internal references
             * that don't point to components/schemas/<SchemaName>
             */
            if (!hasAllOfMapping && (metaRefOrigin !== baseURI || !metaSchemaName && metaRefFields)) {
              let hasMatchingMapping = false;
              mapping.forEach((mappingValue, mappingKey) => {
                var _mappingValueSchema$g;
                const mappingValueSchema = mappingValue.getMetaProperty('ref-schema');
                const mappingValueSchemaRefBaseURI = mappingValueSchema == null || (_mappingValueSchema$g = mappingValueSchema.getMetaProperty('ref-fields')) == null ? void 0 : _mappingValueSchema$g.get('$refBaseURI');
                if (mappingValueSchemaRefBaseURI != null && mappingValueSchemaRefBaseURI.equals(metaRefFields == null ? void 0 : metaRefFields.$refBaseURI)) {
                  normalizedMapping.set((0, _apidomCore.toValue)(mappingKey), (0, _apidomCore.cloneShallow)(item));
                  hasMatchingMapping = true;
                }
              });
              if (!hasMatchingMapping) {
                isNormalized = false;
              }
              return;
            }

            // handle internal references that point to components/schemas/<SchemaName>
            if (metaSchemaName) {
              let hasMatchingMapping = false;
              mapping.forEach((mappingValue, mappingKey) => {
                var _mappingValueSchema$g2;
                const mappingValueSchema = mappingValue.getMetaProperty('ref-schema');
                const mappingValueSchemaName = mappingValueSchema == null ? void 0 : mappingValueSchema.getMetaProperty('schemaName');
                const mappingValueSchemaRefBaseURI = mappingValueSchema == null || (_mappingValueSchema$g2 = mappingValueSchema.getMetaProperty('ref-fields')) == null ? void 0 : _mappingValueSchema$g2.get('$refBaseURI');
                if (mappingValueSchemaName != null && mappingValueSchemaName.equals(metaSchemaName) && (!hasAllOfMapping || mappingValueSchemaRefBaseURI != null && mappingValueSchemaRefBaseURI.equals(metaRefFields == null ? void 0 : metaRefFields.$refBaseURI))) {
                  normalizedMapping.set((0, _apidomCore.toValue)(mappingKey), (0, _apidomCore.cloneShallow)(item));
                  hasMatchingMapping = true;
                }
              });

              // add a new mapping if no matching mapping was found
              if (!hasMatchingMapping) {
                normalizedMapping.set(metaSchemaName, (0, _apidomCore.cloneShallow)(item));
              }
            }
          });

          // check if any mapping is not a Schema Object or if any mapping was not normalized
          const mappingKeys = mapping.keys();
          const normalizedMappingKeys = normalizedMapping.keys();
          isNormalized = isNormalized && normalizedMapping.filter(mappingValue => !(0, _predicates.isSchemaElement)(mappingValue)).length === 0 && mappingKeys.every(mappingKey => normalizedMappingKeys.includes(mappingKey));
          if (isNormalized) {
            schemaElement.discriminator.set('x-normalized-mapping', normalizedMapping);

            // dive in and eliminate cycles that might be created by normalization
            (0, _apidomCore.visit)(schemaElement, {}, {
              // @ts-ignore
              detectCyclesCallback: (node, nodeKey, nodeParent) => {
                if (!nodeParent || !(0, _apidomCore.isMemberElement)(node) || !(0, _apidomCore.isStringElement)(node.key) || !node.key.equals('discriminator') || !(0, _apidomNsOpenapi.isDiscriminatorElement)(node.value)) {
                  return;
                }
                const discriminator = (0, _apidomCore.cloneShallow)(node.value);
                const discriminatorCopy = new _Discriminator.default();
                if (discriminator.get('mapping')) {
                  discriminatorCopy.mapping = discriminator.get('mapping');
                }
                if (discriminator.get('propertyName')) {
                  discriminatorCopy.propertyName = discriminator.get('propertyName');
                }

                // eslint-disable-next-line no-param-reassign
                nodeParent[nodeKey] = new _apidomCore.MemberElement(new _apidomCore.StringElement('discriminator'), discriminatorCopy);
              }
            });
            storage.append(schemaJSONPointer);
          }
        }
      }
    }
  };
};
var _default = exports.default = plugin;