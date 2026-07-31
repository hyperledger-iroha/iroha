import { $defsVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { AlternatingVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { AlternatingVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { CallbackElement as CallbackElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { CallbackVisitor as CallbackVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { CallbackVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { Class } from 'ts-mixer/dist/types/types.js';
import { ComponentsCallbacksElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsCallbacksVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsCallbacksVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsElement as ComponentsElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsExamplesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsExamplesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsExamplesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsHeadersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsHeadersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsHeadersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsLinksElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsLinksVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsLinksVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsParametersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsParametersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsParametersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsRequestBodiesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsRequestBodiesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsRequestBodiesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsResponsesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsResponsesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsResponsesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsSchemasElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsSecuritySchemesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsSecuritySchemesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsSecuritySchemesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsVisitor as ComponentsVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ComponentsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ContactElement as ContactElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ContactVisitor as ContactVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ContactVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ContentVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ContentVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { DefinitionsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependentSchemasVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { DiscriminatorElement as DiscriminatorElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { DiscriminatorMappingElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { DiscriminatorMappingVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { DiscriminatorMappingVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { DiscriminatorVisitor as DiscriminatorVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { DiscriminatorVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import type { ElementPredicate } from '@swagger-api/apidom-core';
import { EncodingElement as EncodingElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { EncodingHeadersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { EncodingHeadersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { EncodingHeadersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { EncodingVisitor as EncodingVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { EncodingVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ExampleElement as ExampleElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExampleExternalValueVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExampleExternalValueVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExamplesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExamplesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExampleVisitor as ExampleVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExampleVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExternalDocumentationElement as ExternalDocumentationElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExternalDocumentationVisitor as ExternalDocumentationVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ExternalDocumentationVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { FallbackVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { FallbackVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { FixedFieldsVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { FixedFieldsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { hasElementSourceMap } from '@swagger-api/apidom-core';
import { HeaderContentElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderContentVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderContentVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderElement as HeaderElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderExamplesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderExamplesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderExamplesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderSchemaVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderSchemaVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderVisitor as HeaderVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { HeaderVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { includesClasses } from '@swagger-api/apidom-core';
import { InfoElement as InfoElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { InfoVersionVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { InfoVersionVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { InfoVisitor as InfoVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { InfoVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isLinkElement as isLinkPrimitiveElement } from '@swagger-api/apidom-core';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isOpenApiExtension } from '@swagger-api/apidom-ns-openapi-3-0';
import { isRefElement } from '@swagger-api/apidom-core';
import { isReferenceLikeElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { isServerLikeElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { isServersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { isStringElement } from '@swagger-api/apidom-core';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { LicenseElement as LicenseElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { LicenseVisitor as LicenseVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { LicenseVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkDescriptionVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkElement as LinkElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkOperationIdVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkOperationIdVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkOperationRefVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkOperationRefVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkParametersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkParametersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkParametersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinksVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkVisitor as LinkVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { LinkVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { MapVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { MapVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeElement as MediaTypeElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeEncodingElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeEncodingVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeEncodingVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeExamplesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeExamplesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeExamplesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MediaTypeSchemaVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeSchemaVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeVisitor as MediaTypeVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { MediaTypeVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { Meta } from '@swagger-api/apidom-core';
import { MixedFieldsVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { MixedFieldsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { Namespace } from 'minim';
import { Namespace as Namespace_2 } from '@swagger-api/apidom-core';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { OAuthFlowElement as OAuthFlowElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowScopesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowScopesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowScopesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowsElement as OAuthFlowsElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowsVisitor as OAuthFlowsVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowVisitor as OAuthFlowVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OAuthFlowVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ObjectElement } from '@swagger-api/apidom-core';
import { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { OpenapiElement as OpenapiElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OpenapiVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OpenapiVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationCallbacksElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationCallbacksVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationCallbacksVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationElement as OperationElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationParametersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationParametersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationParametersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationRequestBodyVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationRequestBodyVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationSecurityElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationSecurityVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationSecurityVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationServersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationServersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationServersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationTagsElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationTagsVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationTagsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationVisitor as OperationVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { OperationVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterContentElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterContentVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterContentVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterElement as ParameterElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterExamplesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterExamplesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterExampleVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterSchemaVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterSchemaVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParametersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParametersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterVisitor as ParameterVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParameterVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { ParentSchemaAwareVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { PathItem$RefVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItem$RefVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemElement as PathItemElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemParametersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemParametersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemParametersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemServersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemServersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemServersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemVisitor as PathItemVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathItemVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathsElement as PathsElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathsVisitor as PathsVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { PathsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { PatternedFieldsVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { PatternedFieldsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { PatternPropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { PrefixItemsVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { Reference$RefVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { Reference$RefVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ReferenceElement as ReferenceElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ReferenceVisitor as ReferenceVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ReferenceVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequestBodyContentElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequestBodyContentVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequestBodyContentVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequestBodyElement as RequestBodyElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequestBodyVisitor as RequestBodyVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequestBodyVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ResponseContentElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseContentVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseContentVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseElement as ResponseElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseHeadersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseHeadersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseHeadersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseLinksElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseLinksVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseLinksVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponsesDefaultVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponsesDefaultVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponsesElement as ResponsesElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponsesVisitor as ResponsesVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponsesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseVisitor as ResponseVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ResponseVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { $defsVisitorOptions as Schema$defsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { $refVisitor as Schema$refVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { $refVisitorOptions as Schema$refVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { $vocabularyVisitor as Schema$vocabularyVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { $vocabularyVisitorOptions as Schema$vocabularyVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { AllOfVisitorOptions as SchemaAllOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { AnyOfVisitorOptions as SchemaAnyOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { DependentRequiredVisitor as SchemaDependentRequiredVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { DependentRequiredVisitorOptions as SchemaDependentRequiredVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { DependentSchemasVisitorOptions as SchemaDependentSchemasVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { OneOfVisitorOptions as SchemaOneOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternPropertiesVisitorOptions as SchemaPatternPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { PrefixItemsVisitorOptions as SchemaPrefixItemsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { PropertiesVisitorOptions as SchemaPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { SecurityElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecurityRequirementElement as SecurityRequirementElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecurityRequirementVisitor as SecurityRequirementVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecurityRequirementVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecuritySchemeElement as SecuritySchemeElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecuritySchemeVisitor as SecuritySchemeVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecuritySchemeVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecurityVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { SecurityVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerElement as ServerElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServersElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServersVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServersVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableElement as ServerVariableElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariablesElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableUrlVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableUrlVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableVariablesVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableVariablesVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableVisitor as ServerVariableVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVariableVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVisitor as ServerVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { ServerVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { SpecificationExtensionVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { SpecificationExtensionVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { SpecificationVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { SpecificationVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { SpecPath } from '@swagger-api/apidom-ns-openapi-3-0';
import { StringElement } from '@swagger-api/apidom-core';
import { TagElement as TagElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { TagsElement } from '@swagger-api/apidom-ns-openapi-3-0';
import { TagsVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { TagsVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { TagVisitor as TagVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { TagVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Visitor } from '@swagger-api/apidom-ns-openapi-3-0';
import { VisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';
import { XmlElement as XmlElement_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { XmlVisitor as XmlVisitor_2 } from '@swagger-api/apidom-ns-openapi-3-0';
import { XmlVisitorOptions } from '@swagger-api/apidom-ns-openapi-3-0';

export { AlternatingVisitor }

export { AlternatingVisitorOptions }

/**
 * Translates visitor ancestor lineage to a JSON Pointer tokens.
 * Ancestor lineage is constructed of following visitor method arguments:
 *
 *  - ancestors
 *  - parent
 *  - element
 * @public
 */
export declare const ancestorLineageToJSONPointer: <T extends (Element_2 | Element_2[])[]>(elementPath: T) => string;

/**
 * @public
 */
export declare const BaseCallbackVisitor: typeof CallbackVisitor_2;

/**
 * @public
 */
export declare const BaseComponentsVisitor: typeof ComponentsVisitor_2;

/**
 * @public
 */
export declare const BaseContactVisitor: typeof ContactVisitor_2;

/**
 * @public
 */
export declare const BaseDiscriminatorVisitor: typeof DiscriminatorVisitor_2;

/**
 * @public
 */
export declare const BaseEncodingVisitor: typeof EncodingVisitor_2;

/**
 * @public
 */
export declare const BaseExampleVisitor: typeof ExampleVisitor_2;

/**
 * @public
 */
export declare const BaseExternalDocumentationVisitor: typeof ExternalDocumentationVisitor_2;

/**
 * @public
 */
export declare const BaseHeaderVisitor: typeof HeaderVisitor_2;

/**
 * @public
 */
export declare const BaseInfoVisitor: typeof InfoVisitor_2;

/**
 * @public
 */
export declare const BaseLicenseVisitor: typeof LicenseVisitor_2;

/**
 * @public
 */
export declare const BaseLinkVisitor: typeof LinkVisitor_2;

/**
 * @public
 */
export declare const BaseMediaTypeVisitor: typeof MediaTypeVisitor_2;

/**
 * @public
 */
export declare const BaseOAuthFlowsVisitor: typeof OAuthFlowsVisitor_2;

/**
 * @public
 */
export declare const BaseOAuthFlowVisitor: typeof OAuthFlowVisitor_2;

/**
 * @public
 */
export declare const BaseOperationVisitor: typeof OperationVisitor_2;

/**
 * @public
 */
export declare const BaseParameterVisitor: typeof ParameterVisitor_2;

/**
 * @public
 */
export declare const BasePathItemVisitor: typeof PathItemVisitor_2;

/**
 * @public
 */
export declare const BasePathsVisitor: typeof PathsVisitor_2;

/**
 * @public
 */
export declare const BaseReferenceVisitor: typeof ReferenceVisitor_2;

/**
 * @public
 */
export declare const BaseRequestBodyVisitor: typeof RequestBodyVisitor_2;

/**
 * @public
 */
export declare const BaseResponsesVisitor: typeof ResponsesVisitor_2;

/**
 * @public
 */
export declare const BaseResponseVisitor: typeof ResponseVisitor_2;

/**
 * @public
 */
export declare const BaseSecurityRequirementVisitor: typeof SecurityRequirementVisitor_2;

/**
 * @public
 */
export declare const BaseSecuritySchemeVisitor: typeof SecuritySchemeVisitor_2;

/**
 * @public
 */
export declare const BaseServerVariableVisitor: typeof ServerVariableVisitor_2;

/**
 * @public
 */
export declare const BaseServerVisitor: typeof ServerVisitor_2;

/**
 * @public
 */
export declare const BaseTagVisitor: typeof TagVisitor_2;

/**
 * @public
 */
export declare const BaseXMLVisitor: typeof XmlVisitor_2;

/**
 * @public
 */
export declare class CallbackElement extends CallbackElement_2 {
}

/**
 * @public
 */
export declare class CallbackVisitor extends BaseCallbackVisitor {
    readonly element: CallbackElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'PathItem']>;
    constructor(options: CallbackVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

export { CallbackVisitorOptions }

export { ComponentsCallbacksElement }

export { ComponentsCallbacksVisitor }

export { ComponentsCallbacksVisitorOptions }

/**
 * @public
 */
export declare class ComponentsElement extends ComponentsElement_2 {
    get pathItems(): ObjectElement | undefined;
    set pathItems(pathItems: ObjectElement | undefined);
}

export { ComponentsExamplesElement }

export { ComponentsExamplesVisitor }

export { ComponentsExamplesVisitorOptions }

export { ComponentsHeadersElement }

export { ComponentsHeadersVisitor }

export { ComponentsHeadersVisitorOptions }

export { ComponentsLinksElement }

export { ComponentsLinksVisitor }

export { ComponentsLinksVisitorOptions }

export { ComponentsParametersElement }

export { ComponentsParametersVisitor }

export { ComponentsParametersVisitorOptions }

/**
 * @public
 */
export declare class ComponentsPathItemsElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsPathItemsVisitor extends PathItemsVisitor_base {
    readonly element: ComponentsPathItemsElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'PathItem']>;
    constructor(options: ComponentsPathItemsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsPathItemsVisitorOptions extends MapVisitorOptions, FallbackVisitorOptions {
}

export { ComponentsRequestBodiesElement }

export { ComponentsRequestBodiesVisitor }

export { ComponentsRequestBodiesVisitorOptions }

export { ComponentsResponsesElement }

export { ComponentsResponsesVisitor }

export { ComponentsResponsesVisitorOptions }

export { ComponentsSchemasElement }

/**
 * @public
 */
export declare class ComponentsSchemasVisitor extends SchemasVisitor_base {
    readonly element: ComponentsSchemasElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Schema']>;
    constructor(options: ComponentsSchemasVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsSchemasVisitorOptions extends MapVisitorOptions, FallbackVisitorOptions {
}

export { ComponentsSecuritySchemesElement }

export { ComponentsSecuritySchemesVisitor }

export { ComponentsSecuritySchemesVisitorOptions }

/**
 * @public
 */
export declare class ComponentsVisitor extends BaseComponentsVisitor {
    readonly element: ComponentsElement;
    constructor(options: ComponentsVisitorOptions);
}

export { ComponentsVisitorOptions }

/**
 * @public
 */
export declare class ContactElement extends ContactElement_2 {
}

/**
 * @public
 */
export declare class ContactVisitor extends BaseContactVisitor {
    readonly element: ContactElement;
    constructor(options: ContactVisitorOptions);
}

export { ContactVisitorOptions }

export { ContentVisitor }

export { ContentVisitorOptions }

/**
 * @public
 */
export declare const createToolbox: () => Toolbox;

/**
 * @public
 */
export declare class DiscriminatorElement extends DiscriminatorElement_2 {
}

export { DiscriminatorMappingElement }

export { DiscriminatorMappingVisitor }

export { DiscriminatorMappingVisitorOptions }

/**
 * @public
 */
export declare class DiscriminatorVisitor extends BaseDiscriminatorVisitor {
    readonly element: DiscriminatorElement;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: DiscriminatorVisitorOptions);
}

export { DiscriminatorVisitorOptions }

/**
 * @public
 */
export declare class EncodingElement extends EncodingElement_2 {
}

export { EncodingHeadersElement }

export { EncodingHeadersVisitor }

export { EncodingHeadersVisitorOptions }

/**
 * @public
 */
export declare class EncodingVisitor extends BaseEncodingVisitor {
    readonly element: EncodingElement;
    constructor(options: EncodingVisitorOptions);
}

export { EncodingVisitorOptions }

/**
 * @public
 */
export declare class ExampleElement extends ExampleElement_2 {
}

export { ExampleExternalValueVisitor }

export { ExampleExternalValueVisitorOptions }

export { ExamplesVisitor }

export { ExamplesVisitorOptions }

/**
 * @public
 */
export declare class ExampleVisitor extends BaseExampleVisitor {
    readonly element: ExampleElement;
    constructor(options: ExampleVisitorOptions);
}

export { ExampleVisitorOptions }

/**
 * @public
 */
export declare class ExternalDocumentationElement extends ExternalDocumentationElement_2 {
}

/**
 * @public
 */
export declare class ExternalDocumentationVisitor extends BaseExternalDocumentationVisitor {
    readonly element: ExternalDocumentationElement;
    constructor(options: ExternalDocumentationVisitorOptions);
}

export { ExternalDocumentationVisitorOptions }

export { FallbackVisitor }

export { FallbackVisitorOptions }

export { FixedFieldsVisitor }

export { FixedFieldsVisitorOptions }

/**
 * @public
 */
export declare type Format = 'generic' | 'json' | 'yaml';

/**
 * @public
 */
export declare const getNodeType: <T extends Element_2>(element: T) => string | undefined;

export { HeaderContentElement }

export { HeaderContentVisitor }

export { HeaderContentVisitorOptions }

/**
 * @public
 */
export declare class HeaderElement extends HeaderElement_2 {
    get schema(): SchemaElement | undefined;
    set schema(schema: SchemaElement | undefined);
}

export { HeaderExamplesElement }

export { HeaderExamplesVisitor }

export { HeaderExamplesVisitorOptions }

export { HeaderSchemaVisitor }

export { HeaderSchemaVisitorOptions }

/**
 * @public
 */
export declare class HeaderVisitor extends BaseHeaderVisitor {
    readonly element: HeaderElement;
    constructor(options: HeaderVisitorOptions);
}

export { HeaderVisitorOptions }

/**
 * @public
 */
export declare class InfoElement extends InfoElement_2 {
    get license(): LicenseElement | undefined;
    set license(licenseElement: LicenseElement | undefined);
    get summary(): StringElement | undefined;
    set summary(summary: StringElement | undefined);
}

export { InfoVersionVisitor }

export { InfoVersionVisitorOptions }

/**
 * @public
 */
export declare class InfoVisitor extends BaseInfoVisitor {
    readonly element: InfoElement;
    constructor(options: InfoVisitorOptions);
}

export { InfoVisitorOptions }

export { isArrayElement }

export { isBooleanElement }

/**
 * @public
 */
export declare const isBooleanJsonSchemaElement: ElementPredicate<BooleanElement>;

/**
 * @public
 */
export declare const isCallbackElement: ElementPredicate<CallbackElement>;

/**
 * @public
 */
export declare const isComponentsElement: ElementPredicate<ComponentsElement>;

/**
 * @public
 */
export declare const isContactElement: ElementPredicate<ContactElement>;

export { isElement }

/**
 * @public
 */
export declare const isExampleElement: ElementPredicate<ExampleElement>;

/**
 * @public
 */
export declare const isExternalDocumentationElement: ElementPredicate<ExternalDocumentationElement>;

/**
 * @public
 */
declare const isHeaderElement: ElementPredicate<HeaderElement>;

/**
 * @public
 */
export declare const isInfoElement: ElementPredicate<InfoElement>;

/**
 * @public
 */
export declare const isJsonSchemaDialectElement: ElementPredicate<JsonSchemaDialectElement>;

/**
 * @public
 */
export declare const isLicenseElement: ElementPredicate<LicenseElement>;

/**
 * @public
 */
export declare const isLinkElement: ElementPredicate<LinkElement>;

export { isLinkPrimitiveElement }

/**
 * @public
 */
export declare const isMediaTypeElement: ElementPredicate<MediaTypeElement>;

export { isMemberElement }

export { isNullElement }

export { isNumberElement }

export { isObjectElement }

/**
 * @public
 */
export declare const isOpenApi3_1Element: ElementPredicate<OpenApi3_1Element>;

/**
 * @public
 */
export declare const isOpenapiElement: ElementPredicate<OpenapiElement>;

export { isOpenApiExtension }

/**
 * @public
 */
export declare const isOperationElement: ElementPredicate<OperationElement>;

/**
 * @public
 */
export declare const isParameterElement: ElementPredicate<ParameterElement>;

/**
 * @public
 */
export declare const isPathItemElement: ElementPredicate<PathItemElement>;

/**
 * @deprecated
 * Determining whether a PathItemElement is external or internal is not possible by just looking
 * at value of the $ref fixed field. The value of the $ref field needs to be resolved in runtime
 * using the referring document as the Base URI.
 * @public
 */
export declare const isPathItemElementExternal: ElementPredicate<PathItemElement>;

/**
 * @public
 */
export declare const isPathsElement: ElementPredicate<PathsElement>;

export { isRefElement }

/**
 * @public
 */
export declare const isReferenceElement: ElementPredicate<ReferenceElement>;

/**
 * @deprecated
 * Determining whether a ReferenceElement is external or internal is not possible by just looking
 * at value of the $ref fixed field. The value of the $ref field needs to be resolved in runtime
 * using the referring document as the Base URI.
 * @public
 */
export declare const isReferenceElementExternal: ElementPredicate<ReferenceElement>;

export { isReferenceLikeElement }

/**
 * @public
 */
export declare const isRequestBodyElement: ElementPredicate<RequestBodyElement>;

/**
 * @public
 */
export declare const isResponseElement: ElementPredicate<ResponseElement>;

/**
 * @public
 */
export declare const isResponsesElement: ElementPredicate<ResponsesElement>;

/**
 * @public
 */
export declare const isSchemaElement: ElementPredicate<SchemaElement>;

/**
 * @public
 */
export declare const isSecurityRequirementElement: ElementPredicate<SecurityRequirementElement>;

/**
 * @public
 */
export declare const isSecuritySchemeElement: ElementPredicate<SecuritySchemeElement>;

/**
 * @public
 */
export declare const isServerElement: ElementPredicate<ServerElement>;

export { isServerLikeElement }

export { isServersElement }

/**
 * @public
 */
export declare const isServerVariableElement: ElementPredicate<ServerVariableElement>;

export { isStringElement }

/**
 * @public
 */
export declare class JsonSchemaDialectElement extends StringElement {
    static default: JsonSchemaDialectElement;
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class JsonSchemaDialectVisitor extends JsonSchemaDialectVisitor_base {
    element: JsonSchemaDialectElement;
    StringElement(stringElement: StringElement): {};
}

declare const JsonSchemaDialectVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface JsonSchemaDialectVisitorOptions extends SpecificationVisitorOptions, FallbackVisitorOptions {
}

/**
 * There are unfortunately two `LinkElement` types. One is from base namespace
 * and the other one if from this namespace. `LinkElement` from base namespace
 * is used extremely rarely so it's almost always safe during traversing
 * to assume that `LinkElement` is element from this namespace.
 *
 * To be 100% sure that currently visiting `LinkElement` is from this namespace
 * use `isLinkElement` predicate from this namespace to assert for it.
 * @public
 */
export declare const keyMap: {
    ObjectElement: string[];
    ArrayElement: string[];
    MemberElement: string[];
    StringElement: never[];
    BooleanElement: never[];
    NumberElement: never[];
    NullElement: never[];
    RefElement: never[];
    LinkElement: never[];
    Annotation: never[];
    Comment: never[];
    ParseResultElement: string[];
    CallbackElement: string[];
    ComponentsElement: string[];
    ContactElement: string[];
    DiscriminatorElement: string[];
    Encoding: string[];
    Example: string[];
    ExternalDocumentationElement: string[];
    HeaderElement: string[];
    InfoElement: string[];
    LicenseElement: string[];
    MediaTypeElement: string[];
    OAuthFlowElement: string[];
    OAuthFlowsElement: string[];
    OpenApi3_1Element: string[];
    OperationElement: string[];
    ParameterElement: string[];
    PathItemElement: string[];
    PathsElement: string[];
    ReferenceElement: string[];
    RequestBodyElement: string[];
    ResponseElement: string[];
    ResponsesElement: string[];
    SchemaElement: string[];
    SecurityRequirementElement: string[];
    SecuritySchemeElement: string[];
    ServerElement: string[];
    ServerVariableElement: string[];
    TagElement: string[];
};

/**
 * @public
 */
export declare class LicenseElement extends LicenseElement_2 {
    get identifier(): StringElement | undefined;
    set identifier(name: StringElement | undefined);
}

/**
 * @public
 */
export declare class LicenseVisitor extends BaseLicenseVisitor {
    readonly element: LicenseElement;
    constructor(options: LicenseVisitorOptions);
}

export { LicenseVisitorOptions }

/**
 * @public
 */
export declare class LinkElement extends LinkElement_2 {
}

export { LinkOperationIdVisitor }

export { LinkOperationIdVisitorOptions }

export { LinkOperationRefVisitor }

export { LinkOperationRefVisitorOptions }

export { LinkParametersElement }

export { LinkParametersVisitor }

export { LinkParametersVisitorOptions }

/**
 * @public
 */
export declare class LinkVisitor extends BaseLinkVisitor {
    readonly element: LinkElement;
    constructor(options: LinkVisitorOptions);
}

export { LinkVisitorOptions }

export { MapVisitor }

export { MapVisitorOptions }

/**
 * @public
 */
export declare class MediaTypeElement extends MediaTypeElement_2 {
    get schema(): SchemaElement | undefined;
    set schema(schema: SchemaElement | undefined);
}

export { MediaTypeEncodingElement }

export { MediaTypeEncodingVisitor }

export { MediaTypeEncodingVisitorOptions }

export { MediaTypeExamplesElement }

export { MediaTypeExamplesVisitor }

export { MediaTypeExamplesVisitorOptions }

/**
 * @public
 */
export declare const mediaTypes: OpenAPIMediaTypes;

export { MediaTypeSchemaVisitor }

export { MediaTypeSchemaVisitorOptions }

/**
 * @public
 */
export declare class MediaTypeVisitor extends BaseMediaTypeVisitor {
    readonly element: MediaTypeElement;
    constructor(options: MediaTypeVisitorOptions);
}

export { MediaTypeVisitorOptions }

export { MixedFieldsVisitor }

export { MixedFieldsVisitorOptions }

/**
 * @public
 */
export declare class OAuthFlowElement extends OAuthFlowElement_2 {
}

export { OAuthFlowScopesElement }

export { OAuthFlowScopesVisitor }

export { OAuthFlowScopesVisitorOptions }

/**
 * @public
 */
export declare class OAuthFlowsElement extends OAuthFlowsElement_2 {
}

/**
 * @public
 */
export declare class OAuthFlowsVisitor extends BaseOAuthFlowsVisitor {
    readonly element: OAuthFlowsElement;
    constructor(options: OAuthFlowsVisitorOptions);
}

export { OAuthFlowsVisitorOptions }

/**
 * @public
 */
export declare class OAuthFlowVisitor extends BaseOAuthFlowVisitor {
    readonly element: OAuthFlowElement;
    constructor(options: OAuthFlowVisitorOptions);
}

export { OAuthFlowVisitorOptions }

/**
 * @public
 */
declare const openApi3_1: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default openApi3_1;

/**
 * @public
 */
export declare class OpenApi3_1Element extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get openapi(): OpenapiElement | undefined;
    set openapi(openapi: OpenapiElement | undefined);
    get info(): InfoElement | undefined;
    set info(info: InfoElement | undefined);
    get jsonSchemaDialect(): StringElement | undefined;
    set jsonSchemaDialect(jsonSchemaDialect: StringElement | undefined);
    get servers(): ArrayElement | undefined;
    set servers(servers: ArrayElement | undefined);
    get paths(): PathsElement | undefined;
    set paths(paths: PathsElement | undefined);
    get components(): ComponentsElement | undefined;
    set components(components: ComponentsElement | undefined);
    get security(): ArrayElement | undefined;
    set security(security: ArrayElement | undefined);
    get tags(): ArrayElement | undefined;
    set tags(tags: ArrayElement | undefined);
    get externalDocs(): ExternalDocumentationElement | undefined;
    set externalDocs(externalDocs: ExternalDocumentationElement | undefined);
    get webhooks(): ObjectElement | undefined;
    set webhooks(webhooks: ObjectElement | undefined);
}

declare namespace openApi3_1Predicates {
    export {
        isCallbackElement,
        isComponentsElement,
        isContactElement,
        isExampleElement,
        isExternalDocumentationElement,
        isHeaderElement,
        isInfoElement,
        isJsonSchemaDialectElement,
        isLicenseElement,
        isLinkElement,
        isOpenapiElement,
        isOpenApi3_1Element,
        isOperationElement,
        isParameterElement,
        isPathItemElement,
        isPathItemElementExternal,
        isPathsElement,
        isReferenceElement,
        isReferenceElementExternal,
        isRequestBodyElement,
        isResponseElement,
        isResponsesElement,
        isSchemaElement,
        isBooleanJsonSchemaElement,
        isSecurityRequirementElement,
        isSecuritySchemeElement,
        isServerElement,
        isServerVariableElement,
        isMediaTypeElement
    }
}
export { openApi3_1Predicates }

/**
 * @public
 */
export declare class OpenApi3_1Visitor extends OpenApi3_1Visitor_base {
    readonly element: OpenApi3_1Element;
    protected readonly specPath: SpecPath<['document', 'objects', 'OpenApi']>;
    protected readonly canSupportSpecificationExtensions: true;
    protected readonly openApiSemanticElement: OpenApi3_1Element;
    constructor(options: OpenApi3_1VisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const OpenApi3_1Visitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OpenApi3_1VisitorOptions extends FixedFieldsVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
export declare class OpenapiElement extends OpenapiElement_2 {
}

/**
 * @public
 */
export declare class OpenAPIMediaTypes extends MediaTypes<string> {
    filterByFormat(format?: Format): string[];
    findBy(version?: string, format?: Format): string;
    latest(format?: Format): string;
}

export { OpenapiVisitor }

export { OpenapiVisitorOptions }

export { OperationCallbacksElement }

export { OperationCallbacksVisitor }

export { OperationCallbacksVisitorOptions }

/**
 * @public
 */
export declare class OperationElement extends OperationElement_2 {
    get requestBody(): RequestBodyElement | ReferenceElement | undefined;
    set requestBody(requestBody: RequestBodyElement | ReferenceElement | undefined);
}

export { OperationParametersElement }

export { OperationParametersVisitor }

export { OperationParametersVisitorOptions }

export { OperationRequestBodyVisitor }

export { OperationRequestBodyVisitorOptions }

export { OperationSecurityElement }

export { OperationSecurityVisitor }

export { OperationSecurityVisitorOptions }

export { OperationServersElement }

export { OperationServersVisitor }

export { OperationServersVisitorOptions }

export { OperationTagsElement }

export { OperationTagsVisitor }

export { OperationTagsVisitorOptions }

/**
 * @public
 */
export declare class OperationVisitor extends BaseOperationVisitor {
    readonly element: OperationElement;
    constructor(options: OperationVisitorOptions);
}

export { OperationVisitorOptions }

export { ParameterContentElement }

export { ParameterContentVisitor }

export { ParameterContentVisitorOptions }

/**
 * @public
 */
export declare class ParameterElement extends ParameterElement_2 {
    get schema(): SchemaElement | undefined;
    set schema(schema: SchemaElement | undefined);
}

export { ParameterExamplesElement }

export { ParameterExamplesVisitorOptions }

export { ParameterExampleVisitor }

export { ParameterSchemaVisitor }

export { ParameterSchemaVisitorOptions }

export { ParametersVisitor }

export { ParametersVisitorOptions }

/**
 * @public
 */
export declare class ParameterVisitor extends BaseParameterVisitor {
    readonly element: ParameterElement;
    constructor(options: ParameterVisitorOptions);
}

export { ParameterVisitorOptions }

export { ParentSchemaAwareVisitorOptions }

export { PathItem$RefVisitor }

export { PathItem$RefVisitorOptions }

/**
 * @public
 */
export declare class PathItemElement extends PathItemElement_2 {
    get GET(): OperationElement;
    set GET(operation: OperationElement | undefined);
    get PUT(): OperationElement;
    set PUT(operation: OperationElement | undefined);
    get POST(): OperationElement;
    set POST(operation: OperationElement | undefined);
    get DELETE(): OperationElement;
    set DELETE(operation: OperationElement | undefined);
    get OPTIONS(): OperationElement;
    set OPTIONS(operation: OperationElement | undefined);
    get HEAD(): OperationElement;
    set HEAD(operation: OperationElement | undefined);
    get PATCH(): OperationElement;
    set PATCH(operation: OperationElement | undefined);
    get TRACE(): OperationElement;
    set TRACE(operation: OperationElement | undefined);
}

export { PathItemParametersElement }

export { PathItemParametersVisitor }

export { PathItemParametersVisitorOptions }

export { PathItemServersElement }

export { PathItemServersVisitor }

export { PathItemServersVisitorOptions }

declare const PathItemsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class PathItemVisitor extends BasePathItemVisitor {
    readonly element: PathItemElement;
    constructor(options: PathItemVisitorOptions);
}

export { PathItemVisitorOptions }

/**
 * @public
 */
export declare class PathsElement extends PathsElement_2 {
}

/**
 * @public
 */
export declare class PathsVisitor extends BasePathsVisitor {
    readonly element: PathsElement;
    constructor(options: PathsVisitorOptions);
}

export { PathsVisitorOptions }

export { PatternedFieldsVisitor }

export { PatternedFieldsVisitorOptions }

export { Reference$RefVisitor }

export { Reference$RefVisitorOptions }

/**
 * @public
 */
export declare class ReferenceElement extends ReferenceElement_2 {
}

/**
 * @public
 */
export declare class ReferenceVisitor extends BaseReferenceVisitor {
    readonly element: ReferenceElement;
    constructor(options: ReferenceVisitorOptions);
}

export { ReferenceVisitorOptions }

/**
 * @public
 */
export declare const refractorPluginNormalizeDiscriminatorMapping: ({ storageField, baseURI }?: RefractorPluginNormalizeDiscriminatorMappingOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(element: OpenApi3_1Element): void;
            leave(): void;
        };
        SchemaElement: {
            leave(schemaElement: SchemaElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        };
    };
};

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
export declare interface RefractorPluginNormalizeDiscriminatorMappingOptions {
    storageField?: string;
    baseURI?: string;
}

/**
 * @public
 */
export declare const refractorPluginNormalizeHeaderExamples: ({ storageField }?: RefractorPluginNormalizeHeaderExamplesOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(element: OpenApi3_1Element): void;
            leave(): void;
        };
        HeaderElement: {
            leave(headerElement: HeaderElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        };
    };
};

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
export declare interface RefractorPluginNormalizeHeaderExamplesOptions {
    storageField?: string;
}

/**
 * @public
 */
export declare const refractorPluginNormalizeOperationIds: ({ storageField, operationIdNormalizer, }?: RefractorPluginNormalizeOperationIdsOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(element: OpenApi3_1Element): void;
            leave(): void;
        };
        PathItemElement: {
            enter(pathItemElement: PathItemElement): void;
            leave(): void;
        };
        OperationElement: {
            enter(operationElement: OperationElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        };
        LinkElement: {
            leave(linkElement: LinkElement): void;
        };
    };
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
export declare interface RefractorPluginNormalizeOperationIdsOptions {
    storageField?: string;
    operationIdNormalizer?: (operationId: string, path: string, method: string) => string;
}

/**
 * @public
 */
export declare const refractorPluginNormalizeParameterExamples: ({ storageField }?: RefractorPluginNormalizeParameterExamplesOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(element: OpenApi3_1Element): void;
            leave(): void;
        };
        ParameterElement: {
            leave(parameterElement: ParameterElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        };
    };
};

/**
 * Override of Schema.example and Schema.examples field inside the Parameter Objects.
 *
 * Parameter Object has two fixed fields:
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
export declare interface RefractorPluginNormalizeParameterExamplesOptions {
    storageField?: string;
}

/**
 * @public
 */
export declare const refractorPluginNormalizeParameters: ({ storageField }?: RefractorPluginNormalizeParametersOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(element: OpenApi3_1Element): void;
            leave(): void;
        };
        PathItemElement: {
            enter(pathItemElement: PathItemElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
            leave(): void;
        };
        OperationElement: {
            leave(operationElement: OperationElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        };
    };
};

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
export declare interface RefractorPluginNormalizeParametersOptions {
    storageField?: string;
}

/**
 * @public
 */
export declare const refractorPluginNormalizeSecurityRequirements: ({ storageField }?: RefractorPluginNormalizeSecurityRequirementsOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(openapiElement: OpenApi3_1Element): void;
            leave(): void;
        };
        OperationElement: {
            leave(operationElement: OperationElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        };
    };
};

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
export declare interface RefractorPluginNormalizeSecurityRequirementsOptions {
    storageField?: string;
}

/**
 * @public
 */
export declare const refractorPluginNormalizeServers: ({ storageField }?: RefractorPluginNormalizeServersOptions) => (toolbox: Toolbox) => {
    visitor: {
        OpenApi3_1Element: {
            enter(openapiElement: OpenApi3_1Element): void;
            leave(): void;
        };
        PathItemElement(pathItemElement: PathItemElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
        OperationElement(operationElement: OperationElement, key: string | number, parent: Element_2 | undefined, path: (string | number)[], ancestors: [Element_2 | Element_2[]]): void;
    };
};

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
export declare interface RefractorPluginNormalizeServersOptions {
    storageField?: string;
}

/**
 * @public
 */
export declare const refractorPluginReplaceEmptyElement: () => ({ predicates }: {
    predicates: ToolboxPredicates;
}) => {
    visitor: {
        StringElement(element: StringElement, key: any, parent: any, path: any, ancestors: any[]): {
            [key: string]: any;
            startPositionRow?: number;
            startPositionColumn?: number;
            startIndex?: number;
            endPositionRow?: number;
            endPositionColumn?: number;
            endIndex?: number;
        } | undefined;
    };
};

export { RequestBodyContentElement }

export { RequestBodyContentVisitor }

export { RequestBodyContentVisitorOptions }

/**
 * @public
 */
export declare class RequestBodyElement extends RequestBodyElement_2 {
}

/**
 * @public
 */
export declare class RequestBodyVisitor extends BaseRequestBodyVisitor {
    readonly element: RequestBodyElement;
    constructor(options: RequestBodyVisitorOptions);
}

export { RequestBodyVisitorOptions }

export { ResponseContentElement }

export { ResponseContentVisitor }

export { ResponseContentVisitorOptions }

/**
 * @public
 */
export declare class ResponseElement extends ResponseElement_2 {
}

export { ResponseHeadersElement }

export { ResponseHeadersVisitor }

export { ResponseHeadersVisitorOptions }

export { ResponseLinksElement }

export { ResponseLinksVisitor }

export { ResponseLinksVisitorOptions }

export { ResponsesDefaultVisitor }

export { ResponsesDefaultVisitorOptions }

/**
 * @public
 */
export declare class ResponsesElement extends ResponsesElement_2 {
}

/**
 * @public
 */
export declare class ResponsesVisitor extends BaseResponsesVisitor {
    readonly element: ResponsesElement;
    constructor(options: ResponsesVisitorOptions);
}

export { ResponsesVisitorOptions }

/**
 * @public
 */
export declare class ResponseVisitor extends BaseResponseVisitor {
    readonly element: ResponseElement;
    constructor(options: ResponseVisitorOptions);
}

export { ResponseVisitorOptions }

/**
 * @public
 */
export declare class Schema$defsVisitor extends $defsVisitor {
    constructor(options: Schema$defsVisitorOptions);
}

export { Schema$defsVisitorOptions }

export { Schema$refVisitor }

export { Schema$refVisitorOptions }

export { Schema$vocabularyVisitor }

export { Schema$vocabularyVisitorOptions }

/**
 * @public
 */
export declare class SchemaAllOfVisitor extends AllOfVisitor {
    constructor(options: SchemaAllOfVisitorOptions);
}

export { SchemaAllOfVisitorOptions }

/**
 * @public
 */
export declare class SchemaAnyOfVisitor extends AnyOfVisitor {
    constructor(options: SchemaAnyOfVisitorOptions);
}

export { SchemaAnyOfVisitorOptions }

export { SchemaDependentRequiredVisitor }

export { SchemaDependentRequiredVisitorOptions }

/**
 * @public
 */
export declare class SchemaDependentSchemasVisitor extends DependentSchemasVisitor {
    constructor(options: SchemaDependentSchemasVisitorOptions);
}

export { SchemaDependentSchemasVisitorOptions }

/**
 * @public
 */
export declare class SchemaElement extends JSONSchemaElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    /**
     * OAS base vocabulary
     *
     * URI: https://github.com/OAI/OpenAPI-Specification/blob/master/versions/3.1.0.md#baseVocabulary
     */
    get discriminator(): DiscriminatorElement | undefined;
    set discriminator(discriminator: DiscriminatorElement | undefined);
    get xml(): XmlElement | undefined;
    set xml(xml: XmlElement | undefined);
    get externalDocs(): ExternalDocumentationElement | undefined;
    set externalDocs(externalDocs: ExternalDocumentationElement | undefined);
    /**
     * @deprecated The example property has been deprecated in favor of the JSON Schema examples keyword. Use of example is discouraged, and later versions of this specification may remove it.
     */
    get example(): Element_2 | undefined;
    set example(example: Element_2 | undefined);
}

/**
 * @public
 */
export declare class SchemaOneOfVisitor extends OneOfVisitor {
    constructor(options: SchemaOneOfVisitorOptions);
}

export { SchemaOneOfVisitorOptions }

/**
 * @public
 */
export declare class SchemaPatternPropertiesVisitor extends PatternPropertiesVisitor {
    constructor(options: SchemaPatternPropertiesVisitorOptions);
}

export { SchemaPatternPropertiesVisitorOptions }

/**
 * @public
 */
export declare class SchemaPrefixItemsVisitor extends PrefixItemsVisitor {
    constructor(options: SchemaPrefixItemsVisitorOptions);
}

export { SchemaPrefixItemsVisitorOptions }

/**
 * @public
 */
export declare class SchemaPropertiesVisitor extends PropertiesVisitor {
    constructor(options: SchemaPropertiesVisitorOptions);
}

export { SchemaPropertiesVisitorOptions }

declare const SchemasVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class SchemaVisitor extends SchemaVisitor_base {
    readonly element: SchemaElement;
    protected readonly jsonSchemaDefaultDialect: JsonSchemaDialectElement;
    constructor(options: SchemaVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
    BooleanElement(booleanElement: BooleanElement): {};
    /**
     * This function depends on some external context, so we need to make sure this function
     * works even when no context is provided like when directly refracting generic Object Element
     * into Schema Element: `SchemaElement.refract(new ObjectElement({ type: 'object' });`
     */
    get defaultDialectIdentifier(): JsonSchemaDialectElement;
    handleDialectIdentifier(objectElement: ObjectElement): void;
    handleSchemaIdentifier(objectElement: ObjectElement): void;
}

declare const SchemaVisitor_base: Class<any[], FixedFieldsVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SchemaVisitorOptions extends FixedFieldsVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

export { SecurityElement }

/**
 * @public
 */
export declare class SecurityRequirementElement extends SecurityRequirementElement_2 {
}

/**
 * @public
 */
export declare class SecurityRequirementVisitor extends BaseSecurityRequirementVisitor {
    readonly element: SecurityRequirementElement;
    constructor(options: SecurityRequirementVisitorOptions);
}

export { SecurityRequirementVisitorOptions }

/**
 * @public
 */
export declare class SecuritySchemeElement extends SecuritySchemeElement_2 {
}

/**
 * @public
 */
export declare class SecuritySchemeVisitor extends BaseSecuritySchemeVisitor {
    readonly element: SecuritySchemeElement;
    constructor(options: SecuritySchemeVisitorOptions);
}

export { SecuritySchemeVisitorOptions }

export { SecurityVisitor }

export { SecurityVisitorOptions }

/**
 * @public
 */
export declare class ServerElement extends ServerElement_2 {
}

export { ServersElement }

export { ServersVisitor }

export { ServersVisitorOptions }

/**
 * @public
 */
export declare class ServerVariableElement extends ServerVariableElement_2 {
}

export { ServerVariablesElement }

export { ServerVariableUrlVisitor }

export { ServerVariableUrlVisitorOptions }

export { ServerVariableVariablesVisitor }

export { ServerVariableVariablesVisitorOptions }

/**
 * @public
 */
export declare class ServerVariableVisitor extends BaseServerVariableVisitor {
    readonly element: ServerVariableElement;
    constructor(options: ServerVariableVisitorOptions);
}

export { ServerVariableVisitorOptions }

/**
 * @public
 */
export declare class ServerVisitor extends BaseServerVisitor {
    readonly element: ServerElement;
    constructor(options: ServerVisitorOptions);
}

export { ServerVisitorOptions }

export { SpecificationExtensionVisitor }

export { SpecificationExtensionVisitorOptions }

/**
 * Specification object allows us to have complete control over visitors
 * when traversing the ApiDOM.
 * Specification also allows us to create amended refractors from
 * existing ones by manipulating it.
 *
 * Note: Specification object allows to use absolute internal JSON pointers.
 * @public
 */
export declare const specificationObj: {
    readonly visitors: {
        readonly value: FallbackVisitor;
        readonly document: {
            readonly objects: {
                readonly OpenApi: {
                    readonly $visitor: typeof OpenApi3_1Visitor;
                    readonly fixedFields: {
                        readonly openapi: OpenapiVisitor;
                        readonly info: {
                            readonly $ref: "#/visitors/document/objects/Info";
                        };
                        readonly jsonSchemaDialect: typeof JsonSchemaDialectVisitor;
                        readonly servers: ServersVisitor;
                        readonly paths: {
                            readonly $ref: "#/visitors/document/objects/Paths";
                        };
                        readonly webhooks: typeof WebhooksVisitor;
                        readonly components: {
                            readonly $ref: "#/visitors/document/objects/Components";
                        };
                        readonly security: SecurityVisitor;
                        readonly tags: TagsVisitor;
                        readonly externalDocs: {
                            readonly $ref: "#/visitors/document/objects/ExternalDocumentation";
                        };
                    };
                };
                readonly Info: {
                    readonly $visitor: typeof InfoVisitor;
                    readonly fixedFields: {
                        readonly title: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly termsOfService: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly contact: {
                            readonly $ref: "#/visitors/document/objects/Contact";
                        };
                        readonly license: {
                            readonly $ref: "#/visitors/document/objects/License";
                        };
                        readonly version: InfoVersionVisitor;
                    };
                };
                readonly Contact: {
                    readonly $visitor: typeof ContactVisitor;
                    readonly fixedFields: {
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly url: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly email: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly License: {
                    readonly $visitor: typeof LicenseVisitor;
                    readonly fixedFields: {
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly identifier: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly url: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Server: {
                    readonly $visitor: typeof ServerVisitor;
                    readonly fixedFields: {
                        readonly url: ServerVariableUrlVisitor;
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly variables: ServerVariableVariablesVisitor;
                    };
                };
                readonly ServerVariable: {
                    readonly $visitor: typeof ServerVariableVisitor;
                    readonly fixedFields: {
                        readonly enum: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly default: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Components: {
                    readonly $visitor: typeof ComponentsVisitor;
                    readonly fixedFields: {
                        readonly schemas: typeof ComponentsSchemasVisitor;
                        readonly responses: ComponentsResponsesVisitor;
                        readonly parameters: ComponentsParametersVisitor;
                        readonly examples: ComponentsExamplesVisitor;
                        readonly requestBodies: ComponentsRequestBodiesVisitor;
                        readonly headers: ComponentsHeadersVisitor;
                        readonly securitySchemes: ComponentsSecuritySchemesVisitor;
                        readonly links: ComponentsLinksVisitor;
                        readonly callbacks: ComponentsCallbacksVisitor;
                        readonly pathItems: typeof ComponentsPathItemsVisitor;
                    };
                };
                readonly Paths: {
                    readonly $visitor: typeof PathsVisitor;
                };
                readonly PathItem: {
                    readonly $visitor: typeof PathItemVisitor;
                    readonly fixedFields: {
                        readonly $ref: PathItem$RefVisitor;
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly get: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly put: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly post: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly delete: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly options: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly head: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly patch: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly trace: {
                            readonly $ref: "#/visitors/document/objects/Operation";
                        };
                        readonly servers: PathItemServersVisitor;
                        readonly parameters: PathItemParametersVisitor;
                    };
                };
                readonly Operation: {
                    readonly $visitor: typeof OperationVisitor;
                    readonly fixedFields: {
                        readonly tags: OperationTagsVisitor;
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly externalDocs: {
                            readonly $ref: "#/visitors/document/objects/ExternalDocumentation";
                        };
                        readonly operationId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly parameters: OperationParametersVisitor;
                        readonly requestBody: OperationRequestBodyVisitor;
                        readonly responses: {
                            readonly $ref: "#/visitors/document/objects/Responses";
                        };
                        readonly callbacks: OperationCallbacksVisitor;
                        readonly deprecated: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly security: OperationSecurityVisitor;
                        readonly servers: OperationServersVisitor;
                    };
                };
                readonly ExternalDocumentation: {
                    readonly $visitor: typeof ExternalDocumentationVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly url: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Parameter: {
                    readonly $visitor: typeof ParameterVisitor;
                    readonly fixedFields: {
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly in: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly required: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly deprecated: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly allowEmptyValue: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly style: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly explode: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly allowReserved: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly schema: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly examples: ParameterExampleVisitor;
                        readonly content: ParameterContentVisitor;
                    };
                };
                readonly RequestBody: {
                    readonly $visitor: typeof RequestBodyVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly content: RequestBodyContentVisitor;
                        readonly required: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly MediaType: {
                    readonly $visitor: typeof MediaTypeVisitor;
                    readonly fixedFields: {
                        readonly schema: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly examples: MediaTypeExamplesVisitor;
                        readonly encoding: MediaTypeEncodingVisitor;
                    };
                };
                readonly Encoding: {
                    readonly $visitor: typeof EncodingVisitor;
                    readonly fixedFields: {
                        readonly contentType: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly headers: EncodingHeadersVisitor;
                        readonly style: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly explode: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly allowReserved: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Responses: {
                    readonly $visitor: typeof ResponsesVisitor;
                    readonly fixedFields: {
                        readonly default: ResponsesDefaultVisitor;
                    };
                };
                readonly Response: {
                    readonly $visitor: typeof ResponseVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly headers: ResponseHeadersVisitor;
                        readonly content: ResponseContentVisitor;
                        readonly links: ResponseLinksVisitor;
                    };
                };
                readonly Callback: {
                    readonly $visitor: typeof CallbackVisitor;
                };
                readonly Example: {
                    readonly $visitor: typeof ExampleVisitor;
                    readonly fixedFields: {
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly value: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly externalValue: ExampleExternalValueVisitor;
                    };
                };
                readonly Link: {
                    readonly $visitor: typeof LinkVisitor;
                    readonly fixedFields: {
                        readonly operationRef: LinkOperationRefVisitor;
                        readonly operationId: LinkOperationIdVisitor;
                        readonly parameters: LinkParametersVisitor;
                        readonly requestBody: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly server: {
                            readonly $ref: "#/visitors/document/objects/Server";
                        };
                    };
                };
                readonly Header: {
                    readonly $visitor: typeof HeaderVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly required: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly deprecated: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly allowEmptyValue: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly style: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly explode: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly allowReserved: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly schema: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly examples: HeaderExamplesVisitor;
                        readonly content: HeaderContentVisitor;
                    };
                };
                readonly Tag: {
                    readonly $visitor: typeof TagVisitor;
                    readonly fixedFields: {
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly externalDocs: {
                            readonly $ref: "#/visitors/document/objects/ExternalDocumentation";
                        };
                    };
                };
                readonly Reference: {
                    readonly $visitor: typeof ReferenceVisitor;
                    readonly fixedFields: {
                        readonly $ref: Reference$RefVisitor;
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly JSONSchema: {
                    readonly $ref: "#/visitors/document/objects/Schema";
                };
                readonly LinkDescription: {
                    readonly $visitor: LinkDescriptionVisitor;
                    readonly fixedFields: {
                        href: {
                            $ref: string;
                        };
                        rel: {
                            $ref: string;
                        };
                        title: {
                            $ref: string;
                        };
                        targetSchema: SchemaOrReferenceVisitor;
                        mediaType: {
                            $ref: string;
                        };
                        method: {
                            $ref: string;
                        };
                        encType: {
                            $ref: string;
                        };
                        schema: SchemaOrReferenceVisitor;
                    };
                };
                readonly Schema: {
                    readonly $visitor: typeof SchemaVisitor;
                    readonly fixedFields: {
                        readonly $defs: typeof Schema$defsVisitor;
                        readonly allOf: typeof SchemaAllOfVisitor;
                        readonly anyOf: typeof SchemaAnyOfVisitor;
                        readonly oneOf: typeof SchemaOneOfVisitor;
                        readonly not: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly if: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly then: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly else: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly dependentSchemas: typeof SchemaDependentSchemasVisitor;
                        readonly prefixItems: typeof SchemaPrefixItemsVisitor;
                        readonly items: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly contains: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly properties: typeof SchemaPropertiesVisitor;
                        readonly patternProperties: typeof SchemaPatternPropertiesVisitor;
                        readonly additionalProperties: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly propertyNames: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly unevaluatedItems: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly unevaluatedProperties: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly contentSchema: {
                            readonly $ref: "#/visitors/document/objects/Schema";
                        };
                        readonly discriminator: {
                            readonly $ref: "#/visitors/document/objects/Discriminator";
                        };
                        readonly xml: {
                            readonly $ref: "#/visitors/document/objects/XML";
                        };
                        readonly externalDocs: {
                            readonly $ref: "#/visitors/document/objects/ExternalDocumentation";
                        };
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly id: {
                            $ref: string;
                        };
                        readonly $schema: {
                            $ref: string;
                        };
                        readonly multipleOf: {
                            $ref: string;
                        };
                        readonly maximum: {
                            $ref: string;
                        };
                        readonly exclusiveMaximum: {
                            $ref: string;
                        };
                        readonly minimum: {
                            $ref: string;
                        };
                        readonly exclusiveMinimum: {
                            $ref: string;
                        };
                        readonly maxLength: {
                            $ref: string;
                        };
                        readonly minLength: {
                            $ref: string;
                        };
                        readonly pattern: {
                            $ref: string;
                        };
                        readonly additionalItems: SchemaOrReferenceVisitor;
                        readonly maxItems: {
                            $ref: string;
                        };
                        readonly minItems: {
                            $ref: string;
                        };
                        readonly uniqueItems: {
                            $ref: string;
                        };
                        readonly maxProperties: {
                            $ref: string;
                        };
                        readonly minProperties: {
                            $ref: string;
                        };
                        readonly required: RequiredVisitor;
                        readonly dependencies: DependenciesVisitor;
                        readonly enum: EnumVisitor;
                        readonly type: TypeVisitor;
                        readonly definitions: DefinitionsVisitor;
                        readonly title: {
                            $ref: string;
                        };
                        readonly description: {
                            $ref: string;
                        };
                        readonly default: {
                            $ref: string;
                        };
                        readonly format: {
                            $ref: string;
                        };
                        readonly base: {
                            $ref: string;
                        };
                        readonly links: LinksVisitor;
                        readonly media: {
                            $ref: string;
                        };
                        readonly readOnly: {
                            $ref: string;
                        };
                    };
                };
                readonly Discriminator: {
                    readonly $visitor: typeof DiscriminatorVisitor;
                    readonly fixedFields: {
                        readonly propertyName: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly mapping: DiscriminatorMappingVisitor;
                    };
                };
                readonly XML: {
                    readonly $visitor: typeof XmlVisitor;
                    readonly fixedFields: {
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly namespace: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly prefix: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly attribute: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly wrapped: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly SecurityScheme: {
                    readonly $visitor: typeof SecuritySchemeVisitor;
                    readonly fixedFields: {
                        readonly type: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly in: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly scheme: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly bearerFormat: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly flows: {
                            readonly $ref: "#/visitors/document/objects/OAuthFlows";
                        };
                        readonly openIdConnectUrl: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly OAuthFlows: {
                    readonly $visitor: typeof OAuthFlowsVisitor;
                    readonly fixedFields: {
                        readonly implicit: {
                            readonly $ref: "#/visitors/document/objects/OAuthFlow";
                        };
                        readonly password: {
                            readonly $ref: "#/visitors/document/objects/OAuthFlow";
                        };
                        readonly clientCredentials: {
                            readonly $ref: "#/visitors/document/objects/OAuthFlow";
                        };
                        readonly authorizationCode: {
                            readonly $ref: "#/visitors/document/objects/OAuthFlow";
                        };
                    };
                };
                readonly OAuthFlow: {
                    readonly $visitor: typeof OAuthFlowVisitor;
                    readonly fixedFields: {
                        readonly authorizationUrl: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly tokenUrl: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly refreshUrl: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly scopes: OAuthFlowScopesVisitor;
                    };
                };
                readonly SecurityRequirement: {
                    readonly $visitor: typeof SecurityRequirementVisitor;
                };
            };
            readonly extension: {
                readonly $visitor: SpecificationExtensionVisitor;
            };
        };
    };
};

export { SpecificationVisitor }

export { SpecificationVisitorOptions }

export { SpecPath }

/**
 * @public
 */
export declare class TagElement extends TagElement_2 {
}

export { TagsElement }

export { TagsVisitor }

export { TagsVisitorOptions }

/**
 * @public
 */
export declare class TagVisitor extends BaseTagVisitor {
    readonly element: TagElement;
    constructor(options: TagVisitorOptions);
}

export { TagVisitorOptions }

/**
 * @public
 */
export declare interface Toolbox {
    predicates: ToolboxPredicates;
    ancestorLineageToJSONPointer: typeof ancestorLineageToJSONPointer;
    namespace: Namespace_2;
}

/**
 * @public
 */
export declare type ToolboxPredicates = typeof openApi3_1Predicates & {
    isElement: typeof isElement;
    isStringElement: typeof isStringElement;
    isArrayElement: typeof isArrayElement;
    isObjectElement: typeof isObjectElement;
    isMemberElement: typeof isMemberElement;
    isServersElement: typeof isServersElement;
    includesClasses: typeof includesClasses;
    hasElementSourceMap: typeof hasElementSourceMap;
};

export { Visitor }

export { VisitorOptions }

/**
 * @public
 */
export declare class WebhooksElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class WebhooksVisitor extends WebhooksVisitor_base {
    readonly element: WebhooksElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'PathItem']>;
    constructor(options: WebhooksVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const WebhooksVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface WebhooksVisitorOptions extends MapVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
export declare class XmlElement extends XmlElement_2 {
}

/**
 * @public
 */
export declare class XmlVisitor extends BaseXMLVisitor {
    readonly element: XmlElement;
    constructor(options: XmlVisitorOptions);
}

export { XmlVisitorOptions }

export { }
