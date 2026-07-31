import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { Class } from 'ts-mixer/dist/types/types.js';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { hasElementSourceMap } from '@swagger-api/apidom-core';
import { includesClasses } from '@swagger-api/apidom-core';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isLinkElement as isLinkPrimitiveElement } from '@swagger-api/apidom-core';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isRefElement } from '@swagger-api/apidom-core';
import { isStringElement } from '@swagger-api/apidom-core';
import { ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MemberElement } from '@swagger-api/apidom-core';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { Namespace as Namespace_2 } from '@swagger-api/apidom-core';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { ObjectElement as ObjectElement_2 } from 'minim';
import { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AllOfVisitorOptions as SchemaAllOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitorOptions as SchemaAnyOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ItemsVisitorOptions as SchemaItemsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { OneOfVisitorOptions as SchemaOneOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor as SchemaOrReferenceVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitorOptions as SchemaPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { TypeVisitorOptions as SchemaTypeVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { StringElement } from '@swagger-api/apidom-core';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';

/**
 * @public
 */
export declare class AlternatingVisitor extends SpecificationVisitor {
    protected alternator: Alternator[];
    constructor({ alternator, ...rest }: AlternatingVisitorOptions);
    enter(element: Element_2): {};
}

/**
 * @public
 */
export declare interface AlternatingVisitorOptions extends SpecificationVisitorOptions {
    readonly alternator: Alternator[];
}

/**
 * @public
 */
export declare type Alternator = {
    predicate: (element: unknown) => boolean;
    specPath: string[];
};

/**
 * @public
 */
export declare class CallbackElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

declare const CallbacksVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const CallbacksVisitor_base_2: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class CallbackVisitor extends CallbackVisitor_base {
    readonly element: CallbackElement;
    protected readonly specPath: SpecPath;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: CallbackVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const CallbackVisitor_base: Class<any[], PatternedFieldsVisitor & FallbackVisitor, typeof PatternedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface CallbackVisitorOptions extends PatternedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsCallbacksElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsCallbacksVisitor extends CallbacksVisitor_base {
    readonly element: ComponentsCallbacksElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Callback']>;
    constructor(options: ComponentsCallbacksVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsCallbacksVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get schemas(): ObjectElement | undefined;
    set schemas(schemas: ObjectElement | undefined);
    get responses(): ObjectElement | undefined;
    set responses(responses: ObjectElement | undefined);
    get parameters(): ObjectElement | undefined;
    set parameters(parameters: ObjectElement | undefined);
    get examples(): ObjectElement | undefined;
    set examples(examples: ObjectElement | undefined);
    get requestBodies(): ObjectElement | undefined;
    set requestBodies(requestBodies: ObjectElement | undefined);
    get headers(): ObjectElement | undefined;
    set headers(headers: ObjectElement | undefined);
    get securitySchemes(): ObjectElement | undefined;
    set securitySchemes(securitySchemes: ObjectElement | undefined);
    get links(): ObjectElement | undefined;
    set links(links: ObjectElement | undefined);
    get callbacks(): ObjectElement | undefined;
    set callbacks(callbacks: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class ComponentsExamplesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsExamplesVisitor extends ExamplesVisitor_base {
    readonly element: ComponentsExamplesElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Example']>;
    constructor(options: ComponentsExamplesVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsExamplesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsHeadersElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsHeadersVisitor extends HeadersVisitor_base {
    readonly element: ComponentsHeadersElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Header']>;
    constructor(options: ComponentsHeadersVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsHeadersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsLinksElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsLinksVisitor extends LinksVisitor_base {
    readonly element: ComponentsLinksElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Link']>;
    constructor(options: ComponentsLinksVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsLinksVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsParametersElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsParametersVisitor extends ParametersVisitor_base {
    readonly element: ComponentsParametersElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Parameter']>;
    constructor(options: ComponentsParametersVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsParametersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsRequestBodiesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsRequestBodiesVisitor extends RequestBodiesVisitor_base {
    readonly element: ComponentsRequestBodiesElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'RequestBody']>;
    constructor(options: ComponentsRequestBodiesVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsRequestBodiesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsResponsesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsResponsesVisitor extends ResponsesVisitor_base {
    readonly element: ComponentsResponsesElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Response']>;
    constructor(options: ComponentsResponsesVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsResponsesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsSchemasElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsSchemasVisitor extends SchemasVisitor_base {
    readonly element: ComponentsSchemasElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Schema']>;
    constructor(options: ComponentsSchemasVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsSchemasVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsSecuritySchemesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsSecuritySchemesVisitor extends SecuritySchemesVisitor_base {
    readonly element: ComponentsSecuritySchemesElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'SecurityScheme']>;
    constructor(options: ComponentsSecuritySchemesVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ComponentsSecuritySchemesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsVisitor extends ComponentsVisitor_base {
    readonly element: ComponentsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Components']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ComponentsVisitorOptions);
}

declare const ComponentsVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ComponentsVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ContactElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get url(): StringElement | undefined;
    set url(url: StringElement | undefined);
    get email(): StringElement | undefined;
    set email(email: StringElement | undefined);
}

/**
 * @public
 */
export declare class ContactVisitor extends ContactVisitor_base {
    readonly element: ContactElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Contact']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ContactVisitorOptions);
}

declare const ContactVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ContactVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ContentVisitor extends ContentVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'MediaType']>;
    constructor(options: ContentVisitorOptions);
}

declare const ContentVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
declare interface ContentVisitorOptions extends MapVisitorOptions, VisitorOptions {
}
export { ContentVisitorOptions }
export { ContentVisitorOptions as HeaderContentVisitorOptions }
export { ContentVisitorOptions as ParameterContentVisitorOptions }
export { ContentVisitorOptions as RequestBodyContentVisitorOptions }
export { ContentVisitorOptions as ResponseContentVisitorOptions }

/**
 * @public
 */
export declare const createRefractor: (specPath: string[]) => (value: unknown, options?: {}) => Element_2;

/**
 * @public
 */
export declare const createToolbox: () => {
    predicates: ToolboxPredicates;
    namespace: Namespace_2;
};

declare const DefaultVisitor_base: Class<any[], AlternatingVisitor & FallbackVisitor, typeof AlternatingVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class DiscriminatorElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get propertyName(): StringElement | undefined;
    set propertyName(propertyName: StringElement | undefined);
    get mapping(): ObjectElement | undefined;
    set mapping(mapping: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class DiscriminatorMappingElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class DiscriminatorMappingVisitor extends MappingVisitor_base {
    readonly element: DiscriminatorMappingElement;
    protected readonly specPath: SpecPath<['value']>;
    constructor(options: DiscriminatorMappingVisitorOptions);
}

/**
 * @public
 */
export declare interface DiscriminatorMappingVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class DiscriminatorVisitor extends DiscriminatorVisitor_base {
    readonly element: DiscriminatorElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Discriminator']>;
    protected canSupportSpecificationExtensions: boolean;
    constructor(options: DiscriminatorVisitorOptions);
}

declare const DiscriminatorVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface DiscriminatorVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class EncodingElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get contentType(): StringElement | undefined;
    set contentType(contentType: StringElement | undefined);
    get headers(): ObjectElement | undefined;
    set headers(headers: ObjectElement | undefined);
    get style(): StringElement | undefined;
    set style(style: StringElement | undefined);
    get explode(): BooleanElement | undefined;
    set explode(explode: BooleanElement | undefined);
    get allowedReserved(): BooleanElement | undefined;
    set allowedReserved(allowedReserved: BooleanElement | undefined);
}

/**
 * @public
 */
export declare class EncodingHeadersElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class EncodingHeadersVisitor extends HeadersVisitor_base_2 {
    readonly element: EncodingHeadersElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Header']>;
    constructor(options: EncodingHeadersVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface EncodingHeadersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class EncodingVisitor extends EncodingVisitor_base_2 {
    readonly element: EncodingElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Encoding']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: EncodingVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const EncodingVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const EncodingVisitor_base_2: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface EncodingVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ExampleElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get summary(): StringElement | undefined;
    set summary(summary: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get value(): Element_2 | undefined;
    set value(value: Element_2 | undefined);
    get externalValue(): StringElement | undefined;
    set externalValue(externalValue: StringElement | undefined);
}

/**
 * @public
 */
export declare class ExampleExternalValueVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class ExamplesVisitor extends ExamplesVisitor_base_2 {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Example']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ExamplesVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ExamplesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const ExamplesVisitor_base_2: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
declare interface ExamplesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}
export { ExamplesVisitorOptions }
export { ExamplesVisitorOptions as HeaderExamplesVisitorOptions }
export { ExamplesVisitorOptions as MediaTypeExamplesVisitorOptions }
export { ExamplesVisitorOptions as ParameterExamplesVisitorOptions }

/**
 * @public
 */
export declare class ExampleVisitor extends ExampleVisitor_base {
    readonly element: ExampleElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Example']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ExampleVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ExampleVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ExampleVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ExternalDocumentationElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get url(): StringElement | undefined;
    set url(url: StringElement | undefined);
}

/**
 * @public
 */
export declare class ExternalDocumentationVisitor extends ExternalDocumentationVisitor_base {
    readonly element: ExternalDocumentationElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'ExternalDocumentation']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ExternalDocumentationVisitorOptions);
}

declare const ExternalDocumentationVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ExternalDocumentationVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class FallbackVisitor extends Visitor {
    enter(element: Element_2): {};
}

/**
 * @public
 */
export declare class FixedFieldsVisitor extends SpecificationVisitor {
    protected specPath: SpecPath;
    protected ignoredFields: string[];
    protected canSupportSpecificationExtensions: boolean;
    protected specificationExtensionPredicate: (element: MemberElement) => boolean;
    constructor({ specPath, ignoredFields, canSupportSpecificationExtensions, specificationExtensionPredicate, ...rest }: FixedFieldsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface FixedFieldsVisitorOptions extends SpecificationVisitorOptions {
    readonly specPath: SpecPath;
    readonly ignoredFields?: string[];
    readonly canSupportSpecificationExtensions?: boolean;
    readonly specificationExtensionPredicate?: typeof isOpenApiExtension;
}

/**
 * @public
 */
export declare type Format = 'generic' | 'json' | 'yaml';

/**
 * @public
 */
export declare const getNodeType: <T extends Element_2>(element: T) => string | undefined;

/**
 * @public
 */
export declare class HeaderContentElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class HeaderContentVisitor extends ContentVisitor {
    readonly element: HeaderContentElement;
    constructor(options: ContentVisitorOptions);
}

/**
 * @public
 */
export declare class HeaderElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get required(): BooleanElement;
    set required(required: BooleanElement | undefined);
    get deprecated(): BooleanElement | undefined;
    set deprecated(deprecated: BooleanElement | undefined);
    get allowEmptyValue(): BooleanElement | undefined;
    set allowEmptyValue(allowEmptyValue: BooleanElement | undefined);
    get style(): StringElement | undefined;
    set style(style: StringElement | undefined);
    get explode(): BooleanElement | undefined;
    set explode(explode: BooleanElement | undefined);
    get allowReserved(): BooleanElement | undefined;
    set allowReserved(allowReserved: BooleanElement | undefined);
    get schema(): SchemaElement | ReferenceElement | undefined;
    set schema(schema: SchemaElement | ReferenceElement | undefined);
    get example(): Element_2 | undefined;
    set example(example: Element_2 | undefined);
    get examples(): ObjectElement | undefined;
    set examples(examples: ObjectElement | undefined);
    get contentProp(): ObjectElement | undefined;
    set contentProp(content: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class HeaderExamplesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class HeaderExamplesVisitor extends ExamplesVisitor {
    readonly element: HeaderExamplesElement;
    constructor(options: ExamplesVisitorOptions);
}

/**
 * @public
 */
export declare class HeaderSchemaVisitor extends SchemaVisitor_base_3 {
    constructor(options: HeaderSchemaVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface HeaderSchemaVisitorOptions extends AlternatingVisitorOptions, VisitorOptions {
}

declare const HeadersVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const HeadersVisitor_base_2: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const HeadersVisitor_base_3: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class HeaderVisitor extends HeaderVisitor_base {
    readonly element: HeaderElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Header']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: HeaderVisitorOptions);
}

declare const HeaderVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface HeaderVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class InfoElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get title(): StringElement | undefined;
    set title(title: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get termsOfService(): StringElement | undefined;
    set termsOfService(tos: StringElement | undefined);
    get contact(): ContactElement | undefined;
    set contact(contactElement: ContactElement | undefined);
    get license(): LicenseElement | undefined;
    set license(licenseElement: LicenseElement | undefined);
    get version(): StringElement | undefined;
    set version(version: StringElement | undefined);
}

/**
 * @public
 */
export declare class InfoVersionVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class InfoVisitor extends InfoVisitor_base {
    readonly element: InfoElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Info']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: InfoVisitorOptions);
}

declare const InfoVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface InfoVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

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

/**
 * @public
 */
export declare const isDiscriminatorElement: ElementPredicate<DiscriminatorElement>;

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
export declare const isOpenApi3_0Element: ElementPredicate<OpenApi3_0Element>;

/**
 * @public
 */
export declare const isOpenapiElement: ElementPredicate<OpenapiElement>;

/**
 * @public
 */
export declare const isOpenApiExtension: (element: MemberElement) => boolean;

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
 * @public
 */
export declare const isPathsElement: ElementPredicate<PathsElement>;

export { isRefElement }

/**
 * @public
 */
export declare const isReferenceElement: ElementPredicate<ReferenceElement>;

/**
 * @public
 */
export declare const isReferenceLikeElement: (element: unknown) => element is ReferenceLikeElement;

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

/**
 * @public
 */
export declare const isServerLikeElement: ElementPredicate<ObjectElement_2>;

/**
 * @public
 */
export declare const isServersElement: ElementPredicate<ServersElement>;

/**
 * @public
 */
export declare const isServerVariableElement: ElementPredicate<ServerVariableElement>;

export { isStringElement }

/**
 * @public
 */
export declare const isTagLikeElement: ElementPredicate<ObjectElement_2>;

/**
 * @public
 */
export declare const JSONSchemaAllOfVisitor: typeof AllOfVisitor;

/**
 * @public
 */
export declare const JSONSchemaAnyOfVisitor: typeof AnyOfVisitor;

/**
 * @public
 */
export declare const JSONSchemaItemsVisitor: typeof ItemsVisitor;

/**
 * @public
 */
export declare const JSONSchemaOneOfVisitor: typeof OneOfVisitor;

/**
 * @public
 */
export declare const JSONSchemaOrJSONReferenceVisitor: typeof SchemaOrReferenceVisitor_2;

/**
 * @public
 */
export declare const JSONSchemaPropertiesVisitor: typeof PropertiesVisitor;

/**
 * @public
 */
export declare const JSONSchemaTypeVisitor: typeof TypeVisitor;

/**
 * There are unfortunately two `LinkElement` types. One is from base namespace
 * and the other one if from this namespace. `LinkElement` from base namespace
 * is used extremely rarely, so it's almost always safe during traversing
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
    OpenApi3_0Element: string[];
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
export declare class LicenseElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get url(): StringElement | undefined;
    set url(url: StringElement | undefined);
}

/**
 * @public
 */
export declare class LicenseVisitor extends LicenseVisitor_base {
    readonly element: LicenseElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'License']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: LicenseVisitorOptions);
}

declare const LicenseVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface LicenseVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class LinkElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get operationRef(): StringElement | undefined;
    set operationRef(operationRef: StringElement | undefined);
    get operationId(): StringElement | undefined;
    set operationId(operationId: StringElement | undefined);
    get operation(): OperationElement | undefined;
    set operation(operation: OperationElement | undefined);
    get parameters(): ObjectElement | undefined;
    set parameters(parameters: ObjectElement | undefined);
    get requestBody(): Element_2 | undefined;
    set requestBody(requestBody: Element_2 | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get server(): ServerElement | undefined;
    set server(server: ServerElement | undefined);
}

/**
 * @public
 */
export declare class LinkOperationIdVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class LinkOperationRefVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class LinkParametersElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class LinkParametersVisitor extends ParametersVisitor_base_3 {
    readonly element: LinkParametersElement;
    protected readonly specPath: SpecPath<['value']>;
    constructor(options: LinkParametersVisitorOptions);
}

/**
 * @public
 */
export declare interface LinkParametersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

declare const LinksVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const LinksVisitor_base_2: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class LinkVisitor extends LinkVisitor_base {
    readonly element: LinkElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Link']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: LinkVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const LinkVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface LinkVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

declare const MappingVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class MapVisitor extends PatternedFieldsVisitor {
    constructor(options: MapVisitorOptions);
}

/**
 * @public
 */
export declare interface MapVisitorOptions extends PatternedFieldsVisitorOptions {
}

/**
 * @public
 */
export declare class MediaTypeElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get schema(): SchemaElement | ReferenceElement | undefined;
    set schema(schema: SchemaElement | ReferenceElement | undefined);
    get example(): Element_2 | undefined;
    set example(example: Element_2 | undefined);
    get examples(): ObjectElement | undefined;
    set examples(examples: ObjectElement | undefined);
    get encoding(): ObjectElement | undefined;
    set encoding(encoding: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class MediaTypeEncodingElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class MediaTypeEncodingVisitor extends EncodingVisitor_base {
    readonly element: MediaTypeEncodingElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Encoding']>;
    constructor(options: MediaTypeEncodingVisitorOptions);
}

/**
 * @public
 */
export declare interface MediaTypeEncodingVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class MediaTypeExamplesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class MediaTypeExamplesVisitor extends ExamplesVisitor {
    readonly element: MediaTypeExamplesElement;
    constructor(options: ExamplesVisitorOptions);
}

/**
 * @public
 */
export declare const mediaTypes: OpenAPIMediaTypes;

/**
 * @public
 */
export declare class MediaTypeSchemaVisitor extends SchemaVisitor_base_2 {
    constructor(options: MediaTypeSchemaVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface MediaTypeSchemaVisitorOptions extends AlternatingVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class MediaTypeVisitor extends MediaTypeVisitor_base {
    readonly element: MediaTypeElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'MediaType']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: MediaTypeVisitorOptions);
}

declare const MediaTypeVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface MediaTypeVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class MixedFieldsVisitor extends MixedFieldsVisitor_base {
    protected specPathFixedFields: SpecPath;
    protected specPathPatternedFields: SpecPath;
    constructor({ specPathFixedFields, specPathPatternedFields, ...rest }: MixedFieldsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const MixedFieldsVisitor_base: Class<any[], FixedFieldsVisitor & PatternedFieldsVisitor, typeof FixedFieldsVisitor & typeof PatternedFieldsVisitor>;

/**
 * @public
 */
export declare interface MixedFieldsVisitorOptions extends PatternedFieldsVisitorOptions {
    readonly specPathFixedFields: SpecPath;
    readonly specPathPatternedFields: SpecPath;
}

/**
 * @public
 */
export declare class OAuthFlowElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get authorizationUrl(): StringElement | undefined;
    set authorizationUrl(authorizationUrl: StringElement | undefined);
    get tokenUrl(): StringElement | undefined;
    set tokenUrl(tokenUrl: StringElement | undefined);
    get refreshUrl(): StringElement | undefined;
    set refreshUrl(refreshUrl: StringElement | undefined);
    get scopes(): ObjectElement | undefined;
    set scopes(scopes: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class OAuthFlowScopesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OAuthFlowScopesVisitor extends ScopesVisitor_base {
    readonly element: OAuthFlowScopesElement;
    protected readonly specPath: SpecPath<['value']>;
    constructor(options: OAuthFlowScopesVisitorOptions);
}

/**
 * @public
 */
export declare interface OAuthFlowScopesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OAuthFlowsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get implicit(): OAuthFlowElement | undefined;
    set implicit(implicit: OAuthFlowElement | undefined);
    get password(): OAuthFlowElement | undefined;
    set password(password: OAuthFlowElement | undefined);
    get clientCredentials(): OAuthFlowElement | undefined;
    set clientCredentials(clientCredentials: OAuthFlowElement | undefined);
    get authorizationCode(): OAuthFlowElement | undefined;
    set authorizationCode(authorizationCode: OAuthFlowElement | undefined);
}

/**
 * @public
 */
export declare class OAuthFlowsVisitor extends OAuthFlowsVisitor_base {
    readonly element: OAuthFlowsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'OAuthFlows']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: OAuthFlowsVisitorOptions);
}

declare const OAuthFlowsVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OAuthFlowsVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OAuthFlowVisitor extends OAuthFlowVisitor_base {
    readonly element: OAuthFlowElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'OAuthFlow']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: OAuthFlowVisitorOptions);
}

declare const OAuthFlowVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OAuthFlowVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
declare const openApi3_0: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default openApi3_0;

/**
 * @public
 */
export declare class OpenApi3_0Element extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get openapi(): OpenapiElement | undefined;
    set openapi(openapi: OpenapiElement | undefined);
    get info(): InfoElement | undefined;
    set info(info: InfoElement | undefined);
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
}

declare namespace openApi3_0Predicates {
    export {
        isCallbackElement,
        isComponentsElement,
        isContactElement,
        isExampleElement,
        isExternalDocumentationElement,
        isHeaderElement,
        isInfoElement,
        isLicenseElement,
        isLinkElement,
        isOpenapiElement,
        isOpenApi3_0Element,
        isOperationElement,
        isParameterElement,
        isPathItemElement,
        isPathsElement,
        isReferenceElement,
        isRequestBodyElement,
        isResponseElement,
        isResponsesElement,
        isSchemaElement,
        isBooleanJsonSchemaElement,
        isSecurityRequirementElement,
        isSecuritySchemeElement,
        isServerElement,
        isServerVariableElement,
        isMediaTypeElement,
        isServersElement,
        isDiscriminatorElement
    }
}
export { openApi3_0Predicates }

/**
 * @public
 */
export declare class OpenApi3_0Visitor extends OpenApi3_0Visitor_base {
    readonly element: OpenApi3_0Element;
    protected readonly specPath: SpecPath<['document', 'objects', 'OpenApi']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: OpenApi3_0VisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const OpenApi3_0Visitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OpenApi3_0VisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OpenapiElement extends StringElement {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OpenAPIMediaTypes extends MediaTypes<string> {
    filterByFormat(format?: Format): string[];
    findBy(version?: string, format?: Format): string;
    latest(format?: Format): string;
}

/**
 * @public
 */
export declare class OpenapiVisitor extends OpenapiVisitor_base {
    element: OpenapiElement;
    StringElement(stringElement: StringElement): {};
}

declare const OpenapiVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OpenapiVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OperationCallbacksElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationCallbacksVisitor extends CallbacksVisitor_base_2 {
    readonly element: OperationCallbacksElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Callback']>;
    constructor(options: OperationCallbacksVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface OperationCallbacksVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OperationElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get tags(): ArrayElement | undefined;
    set tags(tags: ArrayElement | undefined);
    get summary(): StringElement | undefined;
    set summary(description: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    set externalDocs(externalDocs: ExternalDocumentationElement | undefined);
    get externalDocs(): ExternalDocumentationElement | undefined;
    get operationId(): StringElement | undefined;
    set operationId(operationId: StringElement | undefined);
    get parameters(): ArrayElement | undefined;
    set parameters(parameters: ArrayElement | undefined);
    get requestBody(): RequestBodyElement | ReferenceElement | undefined;
    set requestBody(requestBody: RequestBodyElement | ReferenceElement | undefined);
    get responses(): ResponsesElement | undefined;
    set responses(responses: ResponsesElement | undefined);
    get callbacks(): ObjectElement | undefined;
    set callbacks(callbacks: ObjectElement | undefined);
    get deprecated(): BooleanElement;
    set deprecated(deprecated: BooleanElement);
    get security(): ArrayElement | undefined;
    set security(security: ArrayElement | undefined);
    get servers(): ArrayElement | undefined;
    set servers(servers: ArrayElement | undefined);
}

/**
 * @public
 */
export declare class OperationParametersElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationParametersVisitor extends ParametersVisitor {
    readonly element: OperationParametersElement;
    constructor(options: ParametersVisitorOptions);
}

/**
 * @public
 */
export declare class OperationRequestBodyVisitor extends AlternatingVisitor {
    constructor(options: OperationRequestBodyVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface OperationRequestBodyVisitorOptions extends AlternatingVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OperationSecurityElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationSecurityVisitor extends SecurityVisitor_base_2 {
    readonly element: OperationSecurityElement;
    constructor(options: OperationSecurityVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface OperationSecurityVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OperationServersElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationServersVisitor extends ServersVisitor {
    readonly element: OperationServersElement;
    constructor(options: ServersVisitorOptions);
}

/**
 * @public
 */
export declare class OperationTagsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationTagsVisitor extends FallbackVisitor {
    element: OperationTagsElement;
    constructor(options: OperationTagsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface OperationTagsVisitorOptions extends VisitorOptions {
}

/**
 * @public
 */
export declare class OperationVisitor extends OperationVisitor_base {
    readonly element: OperationElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Operation']>;
    constructor(options: OperationVisitorOptions);
}

declare const OperationVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OperationVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ParameterContentElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ParameterContentVisitor extends ContentVisitor {
    readonly element: ParameterContentElement;
    constructor(options: ContentVisitorOptions);
}

/**
 * @public
 */
export declare class ParameterElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get in(): StringElement | undefined;
    set in(val: StringElement | undefined);
    get required(): BooleanElement;
    set required(required: BooleanElement);
    get deprecated(): BooleanElement;
    set deprecated(deprecated: BooleanElement);
    get allowEmptyValue(): BooleanElement | undefined;
    set allowEmptyValue(allowEmptyValue: BooleanElement | undefined);
    get style(): StringElement | undefined;
    set style(style: StringElement | undefined);
    get explode(): BooleanElement | undefined;
    set explode(explode: BooleanElement | undefined);
    get allowReserved(): BooleanElement | undefined;
    set allowReserved(allowReserved: BooleanElement | undefined);
    get schema(): SchemaElement | ReferenceElement | undefined;
    set schema(schema: SchemaElement | ReferenceElement | undefined);
    get example(): Element_2 | undefined;
    set example(example: Element_2 | undefined);
    get examples(): ObjectElement | undefined;
    set examples(examples: ObjectElement | undefined);
    get contentProp(): ObjectElement | undefined;
    set contentProp(content: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class ParameterExamplesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ParameterExampleVisitor extends ExamplesVisitor {
    readonly element: ParameterExamplesElement;
    constructor(options: ExamplesVisitorOptions);
}

/**
 * @public
 */
export declare class ParameterSchemaVisitor extends SchemaVisitor_base {
    constructor(options: ParameterSchemaVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ParameterSchemaVisitorOptions extends AlternatingVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ParametersVisitor extends ParametersVisitor_base_2 {
    readonly element: ArrayElement;
    constructor(options: ParametersVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const ParametersVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const ParametersVisitor_base_2: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const ParametersVisitor_base_3: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
declare interface ParametersVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}
export { ParametersVisitorOptions as OperationParametersVisitorOptions }
export { ParametersVisitorOptions }
export { ParametersVisitorOptions as PathItemParametersVisitorOptions }

/**
 * @public
 */
export declare class ParameterVisitor extends ParameterVisitor_base {
    readonly element: ParameterElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Parameter']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ParameterVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ParameterVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ParameterVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class PathItem$RefVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class PathItemElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get $ref(): StringElement | undefined;
    set $ref($ref: StringElement | undefined);
    get summary(): StringElement | undefined;
    set summary(summary: StringElement | undefined);
    get description(): StringElement;
    set description(description: StringElement | undefined);
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
    get servers(): ArrayElement | undefined;
    set servers(servers: ArrayElement | undefined);
    get parameters(): ArrayElement;
    set parameters(parameters: ArrayElement | undefined);
}

/**
 * @public
 */
export declare class PathItemParametersElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class PathItemParametersVisitor extends ParametersVisitor {
    readonly element: PathItemParametersElement;
    constructor(options: ParametersVisitorOptions);
}

/**
 * @public
 */
export declare class PathItemServersElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class PathItemServersVisitor extends ServersVisitor {
    readonly element: PathItemServersElement;
    constructor(options: ServersVisitorOptions);
}

/**
 * @public
 */
export declare class PathItemVisitor extends PathItemVisitor_base {
    readonly element: PathItemElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'PathItem']>;
    constructor(options: PathItemVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const PathItemVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PathItemVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class PathsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class PathsVisitor extends PathsVisitor_base {
    readonly element: PathsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'PathItem']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: PathsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const PathsVisitor_base: Class<any[], PatternedFieldsVisitor & FallbackVisitor, typeof PatternedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PathsVisitorOptions extends PatternedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class PatternedFieldsVisitor extends SpecificationVisitor {
    protected specPath: SpecPath;
    protected ignoredFields: string[];
    protected fieldPatternPredicate: (value: unknown) => boolean;
    protected canSupportSpecificationExtensions: boolean;
    protected specificationExtensionPredicate: (element: MemberElement) => boolean;
    constructor({ specPath, ignoredFields, fieldPatternPredicate, canSupportSpecificationExtensions, specificationExtensionPredicate, ...rest }: PatternedFieldsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface PatternedFieldsVisitorOptions extends SpecificationVisitorOptions {
    readonly specPath: SpecPath;
    readonly ignoredFields?: string[];
    readonly fieldPatternPredicate?: (...args: unknown[]) => boolean;
    readonly canSupportSpecificationExtensions?: boolean;
    readonly specificationExtensionPredicate?: typeof isOpenApiExtension;
}

/**
 * @public
 */
export declare class Reference$RefVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class ReferenceElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get $ref(): StringElement | undefined;
    set $ref($ref: StringElement | undefined);
}

/**
 * @public
 */
export declare interface ReferenceLikeElement extends ObjectElement_2 {
    hasKey: (value: '$ref') => true;
}

/**
 * @public
 */
export declare class ReferenceVisitor extends ReferenceVisitor_base {
    readonly element: ReferenceElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Reference']>;
    protected readonly canSupportSpecificationExtensions: false;
    constructor(options: ReferenceVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ReferenceVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ReferenceVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare const refract: <T extends Element_2>(value: unknown, { specPath, plugins }?: {
    specPath?: string[] | undefined;
    plugins?: never[] | undefined;
}) => T;

/**
 * @public
 */
export declare const refractorPluginReplaceEmptyElement: () => () => {
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

declare const RequestBodiesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class RequestBodyContentElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class RequestBodyContentVisitor extends ContentVisitor {
    readonly element: RequestBodyContentElement;
    constructor(options: ContentVisitorOptions);
}

/**
 * @public
 */
export declare class RequestBodyElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get contentProp(): ObjectElement | undefined;
    set contentProp(content: ObjectElement | undefined);
    get required(): BooleanElement;
    set required(required: BooleanElement);
}

/**
 * @public
 */
export declare class RequestBodyVisitor extends RequestBodyVisitor_base {
    readonly element: RequestBodyElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'RequestBody']>;
    constructor(options: RequestBodyVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const RequestBodyVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface RequestBodyVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ResponseContentElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ResponseContentVisitor extends ContentVisitor {
    readonly element: ResponseContentElement;
    constructor(options: ContentVisitorOptions);
}

/**
 * @public
 */
export declare class ResponseElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get headers(): ObjectElement | undefined;
    set headers(headers: ObjectElement | undefined);
    get contentProp(): ObjectElement | undefined;
    set contentProp(contentProp: ObjectElement | undefined);
    get links(): ObjectElement | undefined;
    set links(links: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class ResponseHeadersElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ResponseHeadersVisitor extends HeadersVisitor_base_3 {
    readonly element: ResponseHeadersElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Header']>;
    constructor(options: ResponseHeadersVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ResponseHeadersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ResponseLinksElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ResponseLinksVisitor extends LinksVisitor_base_2 {
    readonly element: ResponseLinksElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Link']>;
    constructor(options: ResponseLinksVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ResponseLinksVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ResponsesDefaultVisitor extends DefaultVisitor_base {
    constructor(options: ResponsesDefaultVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface ResponsesDefaultVisitorOptions extends AlternatingVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ResponsesElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get default(): ResponseElement | ReferenceElement | undefined;
    set default(defaultValue: ResponseElement | ReferenceElement | undefined);
}

/**
 * @public
 */
export declare class ResponsesVisitor extends ResponsesVisitor_base_2 {
    readonly element: ResponsesElement;
    protected readonly specPathFixedFields: SpecPath<['document', 'objects', 'Responses']>;
    protected readonly canSupportSpecificationExtensions: true;
    protected readonly specPathPatternedFields: SpecPath<[
    'document',
    'objects',
    'Reference'
    ] | ['document', 'objects', 'Response']>;
    constructor(options: ResponsesVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ResponsesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const ResponsesVisitor_base_2: Class<any[], MixedFieldsVisitor & FallbackVisitor, typeof MixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ResponsesVisitorOptions extends MixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ResponseVisitor extends ResponseVisitor_base {
    readonly element: ResponseElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Response']>;
    constructor(options: ResponseVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ResponseVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ResponseVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SchemaAllOfVisitor extends JSONSchemaAllOfVisitor {
    ArrayElement(arrayElement: ArrayElement): {};
}

export { SchemaAllOfVisitorOptions }

/**
 * @public
 */
export declare class SchemaAnyOfVisitor extends JSONSchemaAnyOfVisitor {
    ArrayElement(arrayElement: ArrayElement): {};
}

export { SchemaAnyOfVisitorOptions }

/**
 * @public
 */
export declare class SchemaElement extends JSONSchemaElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    /**
     * Core vocabulary
     *
     * URI: https://tools.ietf.org/html/draft-wright-json-schema-00
     */
    get idProp(): StringElement | undefined;
    set idProp(idProps: StringElement | undefined);
    get $schema(): StringElement | undefined;
    set $schema($schema: StringElement | undefined);
    /**
     * Validation keywords for arrays
     */
    get additionalItems(): this | ReferenceElement | BooleanElement | undefined;
    set additionalItems(additionalItems: this | ReferenceElement | BooleanElement | undefined);
    get items(): this | ReferenceElement | undefined;
    set items(items: this | ReferenceElement | undefined);
    /**
     * Validation keywords for objects
     */
    get additionalProperties(): this | ReferenceElement | BooleanElement | undefined;
    set additionalProperties(additionalProperties: this | ReferenceElement | BooleanElement | undefined);
    get patternProperties(): ObjectElement | undefined;
    set patternProperties(patternProperties: ObjectElement | undefined);
    get dependencies(): ObjectElement | undefined;
    set dependencies(dependencies: ObjectElement | undefined);
    /**
     *  Validation keywords for any instance type
     */
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get not(): this | ReferenceElement | undefined;
    set not(not: this | ReferenceElement | undefined);
    get definitions(): ObjectElement | undefined;
    set definitions(definitions: ObjectElement | undefined);
    /**
     * JSON Hyper-Schema
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00
     */
    get base(): StringElement | undefined;
    set base(base: StringElement | undefined);
    get links(): ArrayElement | undefined;
    set links(links: ArrayElement | undefined);
    get media(): MediaElement | undefined;
    set media(media: MediaElement | undefined);
    /**
     * OpenAPI vocabulary
     */
    get nullable(): BooleanElement | undefined;
    set nullable(nullable: BooleanElement | undefined);
    get discriminator(): DiscriminatorElement | undefined;
    set discriminator(discriminator: DiscriminatorElement | undefined);
    get writeOnly(): BooleanElement | undefined;
    set writeOnly(writeOnly: BooleanElement | undefined);
    get xml(): XmlElement | undefined;
    set xml(xml: XmlElement | undefined);
    get externalDocs(): ExternalDocumentationElement | undefined;
    set externalDocs(externalDocs: ExternalDocumentationElement | undefined);
    get example(): Element_2 | undefined;
    set example(example: Element_2 | undefined);
    get deprecated(): BooleanElement | undefined;
    set deprecated(deprecated: BooleanElement | undefined);
}

/**
 * @public
 */
export declare class SchemaItemsVisitor extends JSONSchemaItemsVisitor {
    ObjectElement(objectElement: ObjectElement): {};
    ArrayElement(arrayElement: ArrayElement): {};
}

export { SchemaItemsVisitorOptions }

/**
 * @public
 */
export declare class SchemaOneOfVisitor extends JSONSchemaOneOfVisitor {
    ArrayElement(arrayElement: ArrayElement): {};
}

export { SchemaOneOfVisitorOptions }

/**
 * @public
 */
export declare class SchemaOrReferenceVisitor extends JSONSchemaOrJSONReferenceVisitor {
    element: SchemaElement | JSONReferenceElement;
    ObjectElement(objectElement: ObjectElement): {};
}

export { SchemaOrReferenceVisitorOptions }

/**
 * @public
 */
export declare class SchemaPropertiesVisitor extends JSONSchemaPropertiesVisitor {
    ObjectElement(objectElement: ObjectElement): {};
}

export { SchemaPropertiesVisitorOptions }

declare const SchemasVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class SchemaTypeVisitor extends JSONSchemaTypeVisitor {
    ArrayElement(arrayElement: ArrayElement): {};
}

export { SchemaTypeVisitorOptions }

/**
 * @public
 */
export declare class SchemaVisitor extends SchemaVisitor_base_4 {
    readonly element: SchemaElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Schema']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: SchemaVisitorOptions);
}

declare const SchemaVisitor_base: Class<any[], AlternatingVisitor & FallbackVisitor, typeof AlternatingVisitor & typeof FallbackVisitor>;

declare const SchemaVisitor_base_2: Class<any[], AlternatingVisitor & FallbackVisitor, typeof AlternatingVisitor & typeof FallbackVisitor>;

declare const SchemaVisitor_base_3: Class<any[], AlternatingVisitor & FallbackVisitor, typeof AlternatingVisitor & typeof FallbackVisitor>;

declare const SchemaVisitor_base_4: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SchemaVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

declare const ScopesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class SecurityElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SecurityRequirementElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SecurityRequirementVisitor extends SecurityRequirementVisitor_base {
    readonly element: SecurityRequirementElement;
    protected readonly specPath: SpecPath<['value']>;
    constructor(options: SecurityRequirementVisitorOptions);
}

declare const SecurityRequirementVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SecurityRequirementVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SecuritySchemeElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get in(): StringElement | undefined;
    set in(inVal: StringElement | undefined);
    get scheme(): StringElement | undefined;
    set scheme(scheme: StringElement | undefined);
    get bearerFormat(): StringElement | undefined;
    set bearerFormat(bearerFormat: StringElement | undefined);
    get flows(): OAuthFlowsElement | undefined;
    set flows(flows: OAuthFlowsElement | undefined);
    get openIdConnectUrl(): StringElement | undefined;
    set openIdConnectUrl(openIdConnectUrl: StringElement | undefined);
}

declare const SecuritySchemesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class SecuritySchemeVisitor extends SecuritySchemeVisitor_base {
    readonly element: SecuritySchemeElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'SecurityScheme']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: SecuritySchemeVisitorOptions);
}

declare const SecuritySchemeVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SecuritySchemeVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SecurityVisitor extends SecurityVisitor_base {
    readonly element: SecurityElement;
    constructor(options: SecurityVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const SecurityVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const SecurityVisitor_base_2: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SecurityVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ServerElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get url(): StringElement | undefined;
    set url(url: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get variables(): ObjectElement | undefined;
    set variables(variables: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class ServersElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ServersVisitor extends ServersVisitor_base {
    readonly element: ServersElement;
    constructor(options: ServersVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const ServersVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
declare interface ServersVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}
export { ServersVisitorOptions as OperationServersVisitorOptions }
export { ServersVisitorOptions as PathItemServersVisitorOptions }
export { ServersVisitorOptions }

/**
 * @public
 */
export declare class ServerVariableElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get enum(): ArrayElement | undefined;
    set enum(value: ArrayElement | undefined);
    get default(): StringElement | undefined;
    set default(value: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
}

/**
 * @public
 */
export declare class ServerVariablesElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ServerVariableUrlVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class ServerVariableVariablesVisitor extends VariablesVisitor_base {
    readonly element: ServerVariablesElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'ServerVariable']>;
    constructor(options: ServerVariableVariablesVisitorOptions);
}

/**
 * @public
 */
export declare interface ServerVariableVariablesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ServerVariableVisitor extends ServerVariableVisitor_base {
    readonly element: ServerVariableElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'ServerVariable']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ServerVariableVisitorOptions);
}

declare const ServerVariableVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ServerVariableVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ServerVisitor extends ServerVisitor_base {
    readonly element: ServerElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Server']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ServerVisitorOptions);
}

declare const ServerVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ServerVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SpecificationExtensionVisitor extends SpecificationVisitor {
    element: MemberElement;
    MemberElement(memberElement: MemberElement): {};
}

/**
 * @public
 */
export declare const specificationObj: {
    readonly visitors: {
        readonly value: typeof FallbackVisitor;
        readonly document: {
            readonly objects: {
                readonly OpenApi: {
                    readonly $visitor: typeof OpenApi3_0Visitor;
                    readonly fixedFields: {
                        readonly openapi: typeof OpenapiVisitor;
                        readonly info: {
                            readonly $ref: "#/visitors/document/objects/Info";
                        };
                        readonly servers: typeof ServersVisitor;
                        readonly paths: {
                            readonly $ref: "#/visitors/document/objects/Paths";
                        };
                        readonly components: {
                            readonly $ref: "#/visitors/document/objects/Components";
                        };
                        readonly security: typeof SecurityVisitor;
                        readonly tags: typeof TagsVisitor;
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
                        readonly termsOfService: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly contact: {
                            readonly $ref: "#/visitors/document/objects/Contact";
                        };
                        readonly license: {
                            readonly $ref: "#/visitors/document/objects/License";
                        };
                        readonly version: typeof InfoVersionVisitor;
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
                        readonly url: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Server: {
                    readonly $visitor: typeof ServerVisitor;
                    readonly fixedFields: {
                        readonly url: typeof ServerVariableUrlVisitor;
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly variables: typeof ServerVariableVariablesVisitor;
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
                        readonly responses: typeof ComponentsResponsesVisitor;
                        readonly parameters: typeof ComponentsParametersVisitor;
                        readonly examples: typeof ComponentsExamplesVisitor;
                        readonly requestBodies: typeof ComponentsRequestBodiesVisitor;
                        readonly headers: typeof ComponentsHeadersVisitor;
                        readonly securitySchemes: typeof ComponentsSecuritySchemesVisitor;
                        readonly links: typeof ComponentsLinksVisitor;
                        readonly callbacks: typeof ComponentsCallbacksVisitor;
                    };
                };
                readonly Paths: {
                    readonly $visitor: typeof PathsVisitor;
                };
                readonly PathItem: {
                    readonly $visitor: typeof PathItemVisitor;
                    readonly fixedFields: {
                        readonly $ref: typeof PathItem$RefVisitor;
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
                        readonly servers: typeof PathItemServersVisitor;
                        readonly parameters: typeof PathItemParametersVisitor;
                    };
                };
                readonly Operation: {
                    readonly $visitor: typeof OperationVisitor;
                    readonly fixedFields: {
                        readonly tags: typeof OperationTagsVisitor;
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
                        readonly parameters: typeof OperationParametersVisitor;
                        readonly requestBody: typeof OperationRequestBodyVisitor;
                        readonly responses: {
                            readonly $ref: "#/visitors/document/objects/Responses";
                        };
                        readonly callbacks: typeof OperationCallbacksVisitor;
                        readonly deprecated: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly security: typeof OperationSecurityVisitor;
                        readonly servers: typeof OperationServersVisitor;
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
                        readonly schema: typeof ParameterSchemaVisitor;
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly examples: typeof ParameterExampleVisitor;
                        readonly content: typeof ParameterContentVisitor;
                    };
                };
                readonly RequestBody: {
                    readonly $visitor: typeof RequestBodyVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly content: typeof RequestBodyContentVisitor;
                        readonly required: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly MediaType: {
                    readonly $visitor: typeof MediaTypeVisitor;
                    readonly fixedFields: {
                        readonly schema: typeof MediaTypeSchemaVisitor;
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly examples: typeof MediaTypeExamplesVisitor;
                        readonly encoding: typeof MediaTypeEncodingVisitor;
                    };
                };
                readonly Encoding: {
                    readonly $visitor: typeof EncodingVisitor;
                    readonly fixedFields: {
                        readonly contentType: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly headers: typeof EncodingHeadersVisitor;
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
                        readonly default: typeof ResponsesDefaultVisitor;
                    };
                };
                readonly Response: {
                    readonly $visitor: typeof ResponseVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly headers: typeof ResponseHeadersVisitor;
                        readonly content: typeof ResponseContentVisitor;
                        readonly links: typeof ResponseLinksVisitor;
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
                        readonly externalValue: typeof ExampleExternalValueVisitor;
                    };
                };
                readonly Link: {
                    readonly $visitor: typeof LinkVisitor;
                    readonly fixedFields: {
                        readonly operationRef: typeof LinkOperationRefVisitor;
                        readonly operationId: typeof LinkOperationIdVisitor;
                        readonly parameters: typeof LinkParametersVisitor;
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
                        readonly schema: typeof HeaderSchemaVisitor;
                        readonly example: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly examples: typeof HeaderExamplesVisitor;
                        readonly content: typeof HeaderContentVisitor;
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
                        readonly $ref: typeof Reference$RefVisitor;
                    };
                };
                readonly JSONSchema: {
                    readonly $ref: "#/visitors/document/objects/Schema";
                };
                readonly JSONReference: {
                    readonly $ref: "#/visitors/document/objects/Reference";
                };
                readonly Schema: {
                    readonly $visitor: typeof SchemaVisitor;
                    readonly fixedFields: {
                        readonly title: {
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
                        readonly enum: EnumVisitor;
                        readonly type: typeof SchemaTypeVisitor;
                        readonly allOf: typeof SchemaAllOfVisitor;
                        readonly anyOf: typeof SchemaAnyOfVisitor;
                        readonly oneOf: typeof SchemaOneOfVisitor;
                        readonly not: typeof SchemaOrReferenceVisitor;
                        readonly items: typeof SchemaItemsVisitor;
                        readonly properties: typeof SchemaPropertiesVisitor;
                        readonly additionalProperties: typeof SchemaOrReferenceVisitor;
                        readonly description: {
                            $ref: string;
                        };
                        readonly format: {
                            $ref: string;
                        };
                        readonly default: {
                            $ref: string;
                        };
                        readonly nullable: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly discriminator: {
                            readonly $ref: "#/visitors/document/objects/Discriminator";
                        };
                        readonly writeOnly: {
                            readonly $ref: "#/visitors/value";
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
                        readonly deprecated: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Discriminator: {
                    readonly $visitor: typeof DiscriminatorVisitor;
                    readonly fixedFields: {
                        readonly propertyName: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly mapping: typeof DiscriminatorMappingVisitor;
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
                        readonly scopes: typeof OAuthFlowScopesVisitor;
                    };
                };
                readonly SecurityRequirement: {
                    readonly $visitor: typeof SecurityRequirementVisitor;
                };
            };
            readonly extension: {
                readonly $visitor: typeof SpecificationExtensionVisitor;
            };
        };
    };
};

/**
 * @public
 */
export declare class SpecificationVisitor extends Visitor {
    protected readonly specObj: typeof specificationObj;
    protected readonly passingOptionsNames: string[];
    protected openApiGenericElement?: ObjectElement;
    protected openApiSemanticElement?: OpenApi3_0Element;
    constructor({ specObj, passingOptionsNames, openApiGenericElement, openApiSemanticElement, ...rest }: SpecificationVisitorOptions);
    retrievePassingOptions(): string[];
    retrieveFixedFields(specPath: string[]): string[];
    retrieveVisitor(specPath: string[]): unknown;
    retrieveVisitorInstance(specPath: string[], options?: {}): Visitor;
    toRefractedElement(specPath: string[], element: any, options?: {}): any;
}

/**
 * This is a base Type for every visitor that does
 * internal look-ups to retrieve other child visitors.
 * @public
 */
declare interface SpecificationVisitorOptions extends VisitorOptions {
    readonly specObj: typeof specificationObj;
    readonly passingOptionsNames?: string[];
    readonly openApiGenericElement?: ObjectElement;
    readonly openApiSemanticElement?: OpenApi3_0Element;
}
export { SpecificationVisitorOptions as SpecificationExtensionVisitorOptions }
export { SpecificationVisitorOptions }

/**
 * @public
 */
export declare type SpecPath<T = string[]> = (element: unknown) => T;

/**
 * @public
 */
export declare class TagElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get externalDocs(): ExternalDocumentationElement | undefined;
    set externalDocs(externalDocs: ExternalDocumentationElement | undefined);
}

/**
 * @public
 */
export declare class TagsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class TagsVisitor extends TagsVisitor_base {
    readonly element: TagsElement;
    constructor(options: TagsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const TagsVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface TagsVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class TagVisitor extends TagVisitor_base {
    readonly element: TagElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Tag']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: TagVisitorOptions);
}

declare const TagVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface TagVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare type ToolboxPredicates = typeof openApi3_0Predicates & {
    isElement: typeof isElement;
    isStringElement: typeof isStringElement;
    isArrayElement: typeof isArrayElement;
    isObjectElement: typeof isObjectElement;
    isMemberElement: typeof isMemberElement;
    includesClasses: typeof includesClasses;
    hasElementSourceMap: typeof hasElementSourceMap;
};

declare const VariablesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class Visitor {
    element: Element_2;
    constructor(options?: VisitorOptions);
    copyMetaAndAttributes(from: Element_2, to: Element_2): void;
}

/**
 * @public
 */
declare interface VisitorOptions {
}
export { VisitorOptions as ExampleExternalValueVisitorOptions }
export { VisitorOptions as FallbackVisitorOptions }
export { VisitorOptions as InfoVersionVisitorOptions }
export { VisitorOptions as LinkOperationIdVisitorOptions }
export { VisitorOptions as LinkOperationRefVisitorOptions }
export { VisitorOptions as PathItem$RefVisitorOptions }
export { VisitorOptions as Reference$RefVisitorOptions }
export { VisitorOptions as ServerVariableUrlVisitorOptions }
export { VisitorOptions }

/**
 * @public
 */
export declare class XmlElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get namespace(): StringElement | undefined;
    set namespace(namespace: StringElement | undefined);
    get prefix(): StringElement | undefined;
    set prefix(prefix: StringElement | undefined);
    get attribute(): BooleanElement | undefined;
    set attribute(attribute: BooleanElement | undefined);
    get wrapped(): BooleanElement | undefined;
    set wrapped(wrapped: BooleanElement | undefined);
}

/**
 * @public
 */
export declare class XmlVisitor extends XmlVisitor_base {
    readonly element: XmlElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'XML']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: XmlVisitorOptions);
}

declare const XmlVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface XmlVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

export { }
