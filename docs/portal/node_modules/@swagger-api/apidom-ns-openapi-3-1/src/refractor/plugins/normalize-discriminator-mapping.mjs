import { cloneShallow, isArrayElement, ObjectElement, StringElement, MemberElement, toValue, visit, isMemberElement, isStringElement } from '@swagger-api/apidom-core';
import { isReferenceLikeElement, isDiscriminatorElement } from '@swagger-api/apidom-ns-openapi-3-0';
import NormalizeStorage from "./normalize-header-examples/NormalizeStorage.mjs";
import { isSchemaElement } from "../../predicates.mjs";
import DiscriminatorElement from "../../elements/Discriminator.mjs";
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
          storage = new NormalizeStorage(element, storageField, 'discriminator-mapping');
          allOfDiscriminatorMapping = (_element$getMetaPrope = element.getMetaProperty('allOfDiscriminatorMapping')) !== null && _element$getMetaPrope !== void 0 ? _element$getMetaPrope : new ObjectElement();
        },
        leave() {
          storage = undefined;
        }
      },
      SchemaElement: {
        leave(schemaElement, key, parent, path, ancestors) {
          var _parentElement$classe, _schemaElement$discri;
          // no Schema.discriminator field present
          if (!isDiscriminatorElement(schemaElement.discriminator)) {
            return;
          }
          const schemaJSONPointer = ancestorLineageToJSONPointer([...ancestors, parent, schemaElement]);

          // skip visiting this Schema Object if it's already normalized
          if (storage.includes(schemaJSONPointer)) {
            return;
          }

          // skip if both oneOf and anyOf are present
          if (isArrayElement(schemaElement.oneOf) && isArrayElement(schemaElement.anyOf)) {
            return;
          }
          const parentElement = ancestors[ancestors.length - 1];
          const schemaName = schemaElement.getMetaProperty('schemaName');
          const allOfMapping = allOfDiscriminatorMapping.getMember(toValue(schemaName));
          const hasAllOfMapping =
          // @ts-ignore
          allOfMapping && !(parentElement !== null && parentElement !== void 0 && (_parentElement$classe = parentElement.classes) !== null && _parentElement$classe !== void 0 && _parentElement$classe.contains('json-schema-allOf'));

          // skip if neither oneOf, anyOf nor allOf is present
          if (!isArrayElement(schemaElement.oneOf) && !isArrayElement(schemaElement.anyOf) && !hasAllOfMapping) {
            return;
          }
          const mapping = (_schemaElement$discri = schemaElement.discriminator.get('mapping')) !== null && _schemaElement$discri !== void 0 ? _schemaElement$discri : new ObjectElement();
          const normalizedMapping = new ObjectElement();
          let isNormalized = true;
          const items = isArrayElement(schemaElement.oneOf) ? schemaElement.oneOf : isArrayElement(schemaElement.anyOf) ? schemaElement.anyOf : allOfMapping.value;
          items.forEach(item => {
            if (!isSchemaElement(item)) {
              return;
            }
            if (isReferenceLikeElement(item)) {
              isNormalized = false;
              return;
            }
            const metaRefFields = toValue(item.getMetaProperty('ref-fields'));
            const metaRefOrigin = toValue(item.getMetaProperty('ref-origin'));
            const metaSchemaName = toValue(item.getMetaProperty('schemaName'));

            /**
             * handle external references and internal references
             * that don't point to components/schemas/<SchemaName>
             */
            if (!hasAllOfMapping && (metaRefOrigin !== baseURI || !metaSchemaName && metaRefFields)) {
              let hasMatchingMapping = false;
              mapping.forEach((mappingValue, mappingKey) => {
                var _mappingValueSchema$g;
                const mappingValueSchema = mappingValue.getMetaProperty('ref-schema');
                const mappingValueSchemaRefBaseURI = mappingValueSchema === null || mappingValueSchema === void 0 || (_mappingValueSchema$g = mappingValueSchema.getMetaProperty('ref-fields')) === null || _mappingValueSchema$g === void 0 ? void 0 : _mappingValueSchema$g.get('$refBaseURI');
                if (mappingValueSchemaRefBaseURI !== null && mappingValueSchemaRefBaseURI !== void 0 && mappingValueSchemaRefBaseURI.equals(metaRefFields === null || metaRefFields === void 0 ? void 0 : metaRefFields.$refBaseURI)) {
                  normalizedMapping.set(toValue(mappingKey), cloneShallow(item));
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
                const mappingValueSchemaName = mappingValueSchema === null || mappingValueSchema === void 0 ? void 0 : mappingValueSchema.getMetaProperty('schemaName');
                const mappingValueSchemaRefBaseURI = mappingValueSchema === null || mappingValueSchema === void 0 || (_mappingValueSchema$g2 = mappingValueSchema.getMetaProperty('ref-fields')) === null || _mappingValueSchema$g2 === void 0 ? void 0 : _mappingValueSchema$g2.get('$refBaseURI');
                if (mappingValueSchemaName !== null && mappingValueSchemaName !== void 0 && mappingValueSchemaName.equals(metaSchemaName) && (!hasAllOfMapping || mappingValueSchemaRefBaseURI !== null && mappingValueSchemaRefBaseURI !== void 0 && mappingValueSchemaRefBaseURI.equals(metaRefFields === null || metaRefFields === void 0 ? void 0 : metaRefFields.$refBaseURI))) {
                  normalizedMapping.set(toValue(mappingKey), cloneShallow(item));
                  hasMatchingMapping = true;
                }
              });

              // add a new mapping if no matching mapping was found
              if (!hasMatchingMapping) {
                normalizedMapping.set(metaSchemaName, cloneShallow(item));
              }
            }
          });

          // check if any mapping is not a Schema Object or if any mapping was not normalized
          const mappingKeys = mapping.keys();
          const normalizedMappingKeys = normalizedMapping.keys();
          isNormalized = isNormalized && normalizedMapping.filter(mappingValue => !isSchemaElement(mappingValue)).length === 0 && mappingKeys.every(mappingKey => normalizedMappingKeys.includes(mappingKey));
          if (isNormalized) {
            schemaElement.discriminator.set('x-normalized-mapping', normalizedMapping);

            // dive in and eliminate cycles that might be created by normalization
            visit(schemaElement, {}, {
              // @ts-ignore
              detectCyclesCallback: (node, nodeKey, nodeParent) => {
                if (!nodeParent || !isMemberElement(node) || !isStringElement(node.key) || !node.key.equals('discriminator') || !isDiscriminatorElement(node.value)) {
                  return;
                }
                const discriminator = cloneShallow(node.value);
                const discriminatorCopy = new DiscriminatorElement();
                if (discriminator.get('mapping')) {
                  discriminatorCopy.mapping = discriminator.get('mapping');
                }
                if (discriminator.get('propertyName')) {
                  discriminatorCopy.propertyName = discriminator.get('propertyName');
                }

                // eslint-disable-next-line no-param-reassign
                nodeParent[nodeKey] = new MemberElement(new StringElement('discriminator'), discriminatorCopy);
              }
            });
            storage.append(schemaJSONPointer);
          }
        }
      }
    }
  };
};
export default plugin;