export { isRefElement, isLinkElement, isMemberElement, isObjectElement, isArrayElement, isBooleanElement, isNullElement, isElement, isNumberElement, isStringElement } from '@swagger-api/apidom-core';
export { default as mediaTypes, JSONSchema201909MediaTypes } from "./media-types.mjs";
// eslint-disable-next-line no-restricted-exports
export { default } from "./namespace.mjs";
export { default as refractorPluginReplaceEmptyElement } from "./refractor/plugins/replace-empty-element.mjs";
export { default as refract, createRefractor } from "./refractor/index.mjs";
export { default as specificationObj } from "./refractor/specification.mjs";
export { isJSONSchemaElement, isLinkDescriptionElement } from "./predicates.mjs";
export { SpecificationVisitor, FallbackVisitor, FixedFieldsVisitor, PatternedFieldsVisitor, MapVisitor, AlternatingVisitor, ParentSchemaAwareVisitor, Visitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
export { default as JSONSchemaVisitor } from "./refractor/visitors/json-schema/index.mjs";
export { default as LinkDescriptionVisitor } from "./refractor/visitors/json-schema/link-description/index.mjs";
export { default as $defsVisitor } from "./refractor/visitors/json-schema/$defsVisitor.mjs";
export { default as $refVisitor } from "./refractor/visitors/json-schema/$refVisitor.mjs";
export { default as $vocabularyVisitor } from "./refractor/visitors/json-schema/$vocabularyVisitor.mjs";
export { default as AllOfVisitor } from "./refractor/visitors/json-schema/AllOfVisitor.mjs";
export { default as AnyOfVisitor } from "./refractor/visitors/json-schema/AnyOfVisitor.mjs";
export { default as DependentRequiredVisitor } from "./refractor/visitors/json-schema/DependentRequiredVisitor.mjs";
export { default as DependentSchemasVisitor } from "./refractor/visitors/json-schema/DependentSchemasVisitor.mjs";
export { default as ItemsVisitor } from "./refractor/visitors/json-schema/ItemsVisitor.mjs";
export { default as OneOfVisitor } from "./refractor/visitors/json-schema/OneOfVisitor.mjs";
export { default as PatternPropertiesVisitor } from "./refractor/visitors/json-schema/PatternPropertiesVisitor.mjs";
export { default as PropertiesVisitor } from "./refractor/visitors/json-schema/PropertiesVisitor.mjs";
export { keyMap, getNodeType } from "./traversal/visitor.mjs";
/**
 * JSON Schema 2019-09 specification elements.
 */
export { JSONSchemaElement, LinkDescriptionElement } from "./refractor/registration.mjs";