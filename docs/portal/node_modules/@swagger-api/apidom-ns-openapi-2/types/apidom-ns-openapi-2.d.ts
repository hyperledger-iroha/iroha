import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { Class } from 'ts-mixer/dist/types/types.js';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isJSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isJSONReferenceLikeElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isLinkElement as isLinkPrimitiveElement } from '@swagger-api/apidom-core';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isRefElement } from '@swagger-api/apidom-core';
import { isStringElement } from '@swagger-api/apidom-core';
import { ItemsVisitor as ItemsVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReference$RefVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MemberElement } from '@swagger-api/apidom-core';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { NumberElement } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AllOfVisitorOptions as SchemaAllOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ItemsVisitorOptions as SchemaItemsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitorOptions as SchemaOrJSONReferenceVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitorOptions as SchemaPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
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
    readonly alternator?: Alternator[];
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
export declare class BasePathVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class ConsumesVisitor extends FallbackVisitor {
    element: SwaggerConsumesElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
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
export declare const createRefractor: (specPath: string[]) => (value: unknown, options?: {}) => Element_2;

declare const DefaultVisitor_base: Class<any[], AlternatingVisitor & FallbackVisitor, typeof AlternatingVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class DefinitionsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class DefinitionsVisitor extends DefinitionsVisitor_base {
    readonly element: DefinitionsElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'JSONReference'
    ] | ['document', 'objects', 'Schema']>;
    constructor(options: DefinitionsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const DefinitionsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface DefinitionsVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ExampleElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ExampleVisitor extends ExampleVisitor_base {
    readonly element: ExampleElement;
    protected readonly specPath: SpecPath<['value']>;
    protected readonly canSupportSpecificationExtensions: false;
    constructor(options: ExampleVisitorOptions);
}

declare const ExampleVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ExampleVisitorOptions extends MapVisitorOptions, VisitorOptions {
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
    readonly specificationExtensionPredicate?: typeof isSwaggerExtension;
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
export declare class HeaderElement extends JSONSchemaElement {
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
    get additionalItems(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalItems(additionalItems: this | JSONReferenceElement | BooleanElement | undefined);
    get items(): this | undefined;
    set items(items: this | undefined);
    /**
     * Validation keywords for objects
     */
    get maxProperties(): NumberElement | undefined;
    set maxProperties(maxProperties: NumberElement | undefined);
    get minProperties(): NumberElement | undefined;
    set minProperties(minProperties: NumberElement | undefined);
    get required(): ArrayElement | undefined;
    set required(required: ArrayElement | undefined);
    get properties(): ObjectElement | undefined;
    set properties(properties: ObjectElement | undefined);
    get additionalProperties(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalProperties(additionalProperties: this | JSONReferenceElement | BooleanElement | undefined);
    get patternProperties(): ObjectElement | undefined;
    set patternProperties(patternProperties: ObjectElement | undefined);
    get dependencies(): ObjectElement | undefined;
    set dependencies(dependencies: ObjectElement | undefined);
    /**
     *  Validation keywords for any instance type
     */
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get allOf(): ArrayElement | undefined;
    set allOf(allOf: ArrayElement | undefined);
    get anyOf(): ArrayElement | undefined;
    set anyOf(anyOf: ArrayElement | undefined);
    get oneOf(): ArrayElement | undefined;
    set oneOf(oneOf: ArrayElement | undefined);
    get not(): this | JSONReferenceElement | undefined;
    set not(not: this | JSONReferenceElement | undefined);
    get definitions(): ObjectElement | undefined;
    set definitions(definitions: ObjectElement | undefined);
    /**
     * Metadata keywords
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6
     */
    get title(): StringElement | undefined;
    set title(title: StringElement | undefined);
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
    get readOnly(): BooleanElement | undefined;
    set readOnly(readOnly: BooleanElement | undefined);
}

/**
 * @public
 */
export declare class HeadersElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class HeadersVisitor extends HeadersVisitor_base {
    readonly element: HeadersElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Header']>;
    protected readonly canSupportSpecificationExtensions: false;
    constructor(options: HeadersVisitorOptions);
}

declare const HeadersVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface HeadersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

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
export declare class HostVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
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
export declare const isContactElement: ElementPredicate<ContactElement>;

/**
 * @public
 */
export declare const isDefinitionsElement: ElementPredicate<DefinitionsElement>;

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
export declare const isHeaderElement: ElementPredicate<HeaderElement>;

/**
 * @public
 */
export declare const isHeadersElement: ElementPredicate<HeadersElement>;

/**
 * @public
 */
export declare const isInfoElement: ElementPredicate<InfoElement>;

/**
 * @public
 */
export declare const isItemsElement: ElementPredicate<ItemsElement>;

export { isJSONReferenceElement }

export { isJSONReferenceLikeElement }

/**
 * @public
 */
export declare const isLicenseElement: ElementPredicate<LicenseElement>;

export { isLinkPrimitiveElement }

export { isMemberElement }

export { isNullElement }

export { isNumberElement }

export { isObjectElement }

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
export declare const isParametersDefinitionsElement: ElementPredicate<ParametersDefinitionsElement>;

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
export declare const isResponseElement: ElementPredicate<ResponseElement>;

/**
 * @public
 */
export declare const isResponsesDefinitionsElement: ElementPredicate<ResponsesDefinitionsElement>;

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
export declare const isScopesElement: ElementPredicate<ScopesElement>;

/**
 * @public
 */
export declare const isSecurityDefinitionsElement: ElementPredicate<SecurityDefinitionsElement>;

/**
 * @public
 */
export declare const isSecurityRequirementElement: ElementPredicate<SecurityRequirementElement>;

/**
 * @public
 */
export declare const isSecuritySchemeElement: ElementPredicate<SecuritySchemeElement>;

export { isStringElement }

/**
 * @public
 */
export declare const isSwaggerElement: ElementPredicate<SwaggerElement>;

/**
 * @public
 */
export declare const isSwaggerExtension: (element: MemberElement) => boolean;

/**
 * @public
 */
export declare const isSwaggerVersionElement: ElementPredicate<SwaggerVersionElement>;

/**
 * @public
 */
export declare const isTagElement: ElementPredicate<TagElement>;

/**
 * @public
 */
export declare const isXmlElement: ElementPredicate<XmlElement>;

/**
 * @public
 */
export declare class ItemsElement extends JSONSchemaElement {
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
    get additionalItems(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalItems(additionalItems: this | JSONReferenceElement | BooleanElement | undefined);
    get items(): this | undefined;
    set items(items: this | undefined);
    /**
     * Validation keywords for objects
     */
    get maxProperties(): NumberElement | undefined;
    set maxProperties(maxProperties: NumberElement | undefined);
    get minProperties(): NumberElement | undefined;
    set minProperties(minProperties: NumberElement | undefined);
    get required(): ArrayElement | undefined;
    set required(required: ArrayElement | undefined);
    get properties(): ObjectElement | undefined;
    set properties(properties: ObjectElement | undefined);
    get additionalProperties(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalProperties(additionalProperties: this | JSONReferenceElement | BooleanElement | undefined);
    get patternProperties(): ObjectElement | undefined;
    set patternProperties(patternProperties: ObjectElement | undefined);
    get dependencies(): ObjectElement | undefined;
    set dependencies(dependencies: ObjectElement | undefined);
    /**
     *  Validation keywords for any instance type
     */
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get allOf(): ArrayElement | undefined;
    set allOf(allOf: ArrayElement | undefined);
    get anyOf(): ArrayElement | undefined;
    set anyOf(anyOf: ArrayElement | undefined);
    get oneOf(): ArrayElement | undefined;
    set oneOf(oneOf: ArrayElement | undefined);
    get not(): this | JSONReferenceElement | undefined;
    set not(not: this | JSONReferenceElement | undefined);
    get definitions(): ObjectElement | undefined;
    set definitions(definitions: ObjectElement | undefined);
    /**
     * Metadata keywords
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6
     */
    get title(): StringElement | undefined;
    set title(title: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
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
    get readOnly(): BooleanElement | undefined;
    set readOnly(readOnly: BooleanElement | undefined);
}

/**
 * @public
 */
export declare class ItemsVisitor extends ItemsVisitor_base {
    readonly element: ItemsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Items']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ItemsVisitorOptions);
}

declare const ItemsVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ItemsVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

export { JSONReferenceElement }

/**
 * @public
 */
export declare const JSONSchemaAllOfVisitor: typeof AllOfVisitor;

/**
 * @public
 */
export declare const JSONSchemaItemsVisitor: typeof ItemsVisitor_2;

/**
 * @public
 */
export declare const JSONSchemaOrJSONReferenceVisitor: typeof SchemaOrReferenceVisitor;

/**
 * @public
 */
export declare const JSONSchemaPropertiesVisitor: typeof PropertiesVisitor;

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
    SwaggerElement: string[];
    InfoElement: string[];
    ContactElement: string[];
    LicenseElement: string[];
    PathsElement: string[];
    PathItemElement: string[];
    OperationElement: string[];
    ExternalDocumentationElement: string[];
    ParameterElement: string[];
    ItemsElement: string[];
    ExampleElement: string[];
    ResponsesElement: string[];
    ResponseElement: string[];
    HeadersElement: string[];
    HeaderElement: string[];
    TagElement: string[];
    ReferenceElement: string[];
    JSONReferenceElement: string[];
    SchemaElement: string[];
    XmlElement: string[];
    DefinitionsElement: string[];
    ParametersDefinitionsElement: string[];
    ResponsesDefinitionsElement: string[];
    SecurityDefinitionsElement: string[];
    SecuritySchemeElement: string[];
    ScopesElement: string[];
    SecurityRequirementElement: string[];
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
export declare const mediaTypes: OpenAPIMediaTypes;

/**
 * @public
 */
export declare class MixedFieldsVisitor extends MixedFieldsVisitor_base {
    protected readonly specPathFixedFields: SpecPath;
    protected readonly specPathPatternedFields: SpecPath;
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
declare const openApi2: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default openApi2;

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
export declare class OperationConsumesElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationConsumesVisitor extends FallbackVisitor {
    element: OperationConsumesElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
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
    get responses(): ResponsesElement | undefined;
    set responses(responses: ResponsesElement | undefined);
    get schemes(): ArrayElement | undefined;
    set schemes(schemes: ArrayElement | undefined);
    get deprecated(): BooleanElement;
    set deprecated(deprecated: BooleanElement);
    get security(): ArrayElement | undefined;
    set security(security: ArrayElement | undefined);
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
export declare class OperationParametersVisitor extends ParametersVisitor_base_2 {
    readonly element: OperationParametersElement;
    constructor(options: OperationParametersVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface OperationParametersVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OperationProducesElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationProducesVisitor extends FallbackVisitor {
    element: OperationProducesElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare class OperationSchemesElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationSchemesVisitor extends FallbackVisitor {
    element: OperationSchemesElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
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
export declare class OperationTagsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class OperationTagsVisitor extends FallbackVisitor {
    element: OperationTagsElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
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
export declare class ParameterElement extends JSONSchemaElement {
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
    get additionalItems(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalItems(additionalItems: this | JSONReferenceElement | BooleanElement | undefined);
    get items(): this | undefined;
    set items(items: this | undefined);
    /**
     * Validation keywords for objects
     */
    get maxProperties(): NumberElement | undefined;
    set maxProperties(maxProperties: NumberElement | undefined);
    get minProperties(): NumberElement | undefined;
    set minProperties(minProperties: NumberElement | undefined);
    get required(): BooleanElement | undefined | any;
    set required(required: BooleanElement | undefined | any);
    get properties(): ObjectElement | undefined;
    set properties(properties: ObjectElement | undefined);
    get additionalProperties(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalProperties(additionalProperties: this | JSONReferenceElement | BooleanElement | undefined);
    get patternProperties(): ObjectElement | undefined;
    set patternProperties(patternProperties: ObjectElement | undefined);
    get dependencies(): ObjectElement | undefined;
    set dependencies(dependencies: ObjectElement | undefined);
    /**
     *  Validation keywords for any instance type
     */
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get allOf(): ArrayElement | undefined;
    set allOf(allOf: ArrayElement | undefined);
    get anyOf(): ArrayElement | undefined;
    set anyOf(anyOf: ArrayElement | undefined);
    get oneOf(): ArrayElement | undefined;
    set oneOf(oneOf: ArrayElement | undefined);
    get not(): this | JSONReferenceElement | undefined;
    set not(not: this | JSONReferenceElement | undefined);
    get definitions(): ObjectElement | undefined;
    set definitions(definitions: ObjectElement | undefined);
    /**
     * Metadata keywords
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6
     */
    get title(): StringElement | undefined;
    set title(title: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    /**
     * Semantic validation with "format"
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-7
     */
    get format(): StringElement | undefined;
    set format(format: StringElement | undefined);
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
    get readOnly(): BooleanElement | undefined;
    set readOnly(readOnly: BooleanElement | undefined);
    /**
     * OpenAPI vocabulary
     */
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get in(): StringElement | undefined;
    set in(val: StringElement | undefined);
    get schema(): SchemaElement | undefined;
    set schema(schema: SchemaElement | undefined);
}

/**
 * @public
 */
export declare class ParametersDefinitionsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ParametersDefinitionsVisitor extends ParametersDefinitionsVisitor_base {
    readonly element: ParametersDefinitionsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Parameter']>;
    constructor(options: ParametersDefinitionsVisitorOptions);
}

declare const ParametersDefinitionsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ParametersDefinitionsVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

declare const ParametersVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const ParametersVisitor_base_2: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class ParameterVisitor extends ParameterVisitor_base {
    readonly element: ParameterElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Parameter']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ParameterVisitorOptions);
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
export declare class PathItemParametersVisitor extends ParametersVisitor_base {
    readonly element: PathItemParametersElement;
    constructor(options: PathItemParametersVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface PathItemParametersVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
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
    readonly specificationExtensionPredicate?: typeof isSwaggerExtension;
}

/**
 * @public
 */
export declare class ProducesVisitor extends FallbackVisitor {
    element: SwaggerProducesElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
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
export declare interface ReferenceLikeElement extends ObjectElement {
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

/**
 * @public
 */
export declare class ResponseElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get schema(): SchemaElement | undefined;
    set schema(schema: SchemaElement | undefined);
    get headers(): HeadersElement | undefined;
    set headers(headers: HeadersElement | undefined);
    get examples(): ExampleElement | undefined;
    set examples(examples: ExampleElement | undefined);
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
export declare class ResponsesDefinitionsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ResponsesDefinitionsVisitor extends ResponsesDefinitionsVisitor_base {
    readonly element: ResponsesDefinitionsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Response']>;
    constructor(options: ResponsesDefinitionsVisitorOptions);
}

declare const ResponsesDefinitionsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ResponsesDefinitionsVisitorOptions extends MapVisitorOptions, VisitorOptions {
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
export declare class ResponsesVisitor extends ResponsesVisitor_base {
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

declare const ResponsesVisitor_base: Class<any[], MixedFieldsVisitor & FallbackVisitor, typeof MixedFieldsVisitor & typeof FallbackVisitor>;

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
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ResponseVisitorOptions);
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
    get additionalItems(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalItems(additionalItems: this | JSONReferenceElement | BooleanElement | undefined);
    /**
     * Validation keywords for objects
     */
    get patternProperties(): ObjectElement | undefined;
    set patternProperties(patternProperties: ObjectElement | undefined);
    get dependencies(): ObjectElement | undefined;
    set dependencies(dependencies: ObjectElement | undefined);
    /**
     *  Validation keywords for any instance type
     */
    get anyOf(): ArrayElement | undefined;
    set anyOf(anyOf: ArrayElement | undefined);
    get oneOf(): ArrayElement | undefined;
    set oneOf(oneOf: ArrayElement | undefined);
    get not(): this | JSONReferenceElement | undefined;
    set not(not: this | JSONReferenceElement | undefined);
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
export declare class SchemaOrJSONReferenceVisitor extends JSONSchemaOrJSONReferenceVisitor {
    element: SchemaElement | JSONReferenceElement;
    ObjectElement(objectElement: ObjectElement): {};
}

export { SchemaOrJSONReferenceVisitorOptions }

/**
 * @public
 */
export declare class SchemaPropertiesVisitor extends JSONSchemaPropertiesVisitor {
    ObjectElement(objectElement: ObjectElement): {};
}

export { SchemaPropertiesVisitorOptions }

/**
 * @public
 */
export declare class SchemaVisitor extends SchemaVisitor_base {
    readonly element: SchemaElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Schema']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: SchemaVisitorOptions);
}

declare const SchemaVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SchemaVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SchemesVisitor extends FallbackVisitor {
    element: SwaggerSchemesElement;
    constructor(options: VisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare class ScopesElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ScopesVisitor extends ScopesVisitor_base {
    readonly element: ScopesElement;
    protected readonly specPath: SpecPath<['value']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ScopesVisitorOptions);
}

declare const ScopesVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ScopesVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SecurityDefinitionsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SecurityDefinitionsVisitor extends SecurityDefinitionsVisitor_base {
    readonly element: SecurityDefinitionsElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'SecurityScheme']>;
    constructor(options: SecurityDefinitionsVisitorOptions);
}

declare const SecurityDefinitionsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SecurityDefinitionsVisitorOptions extends MapVisitorOptions, VisitorOptions {
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
    get flow(): StringElement | undefined;
    set flow(flow: StringElement | undefined);
    get authorizationUrl(): StringElement | undefined;
    set authorizationUrl(authorizationUrl: StringElement | undefined);
    get tokenUrl(): StringElement | undefined;
    set tokenUrl(tokenUrl: StringElement | undefined);
    get scopes(): ArrayElement | undefined;
    set scopes(scopes: ArrayElement | undefined);
}

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
    readonly element: SwaggerSecurityElement;
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
export declare class SpecificationExtensionVisitor extends SpecificationVisitor {
    element: MemberElement;
    MemberElement(memberElement: MemberElement): {};
}

/**
 * @public
 */
export declare const specificationObj: {
    visitors: {
        value: typeof FallbackVisitor;
        document: {
            objects: {
                JSONReference: {
                    $visitor: JSONReferenceVisitor;
                    fixedFields: {
                        $ref: JSONReference$RefVisitor;
                    };
                };
                JSONSchema: {
                    $ref: string;
                };
                Swagger: {
                    $visitor: typeof SwaggerVisitor;
                    fixedFields: {
                        swagger: typeof SwaggerVersionVisitor;
                        info: {
                            $ref: string;
                        };
                        host: typeof HostVisitor;
                        basePath: typeof BasePathVisitor;
                        schemes: typeof SchemesVisitor;
                        consumes: typeof ConsumesVisitor;
                        produces: typeof ProducesVisitor;
                        paths: {
                            $ref: string;
                        };
                        definitions: {
                            $ref: string;
                        };
                        parameters: {
                            $ref: string;
                        };
                        responses: {
                            $ref: string;
                        };
                        securityDefinitions: {
                            $ref: string;
                        };
                        security: typeof SecurityVisitor;
                        tags: typeof TagsVisitor;
                        externalDocs: {
                            $ref: string;
                        };
                    };
                };
                Info: {
                    $visitor: typeof InfoVisitor;
                    fixedFields: {
                        title: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        termsOfService: {
                            $ref: string;
                        };
                        contact: {
                            $ref: string;
                        };
                        license: {
                            $ref: string;
                        };
                        version: typeof InfoVersionVisitor;
                    };
                };
                Contact: {
                    $visitor: typeof ContactVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        url: {
                            $ref: string;
                        };
                        email: {
                            $ref: string;
                        };
                    };
                };
                License: {
                    $visitor: typeof LicenseVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        url: {
                            $ref: string;
                        };
                    };
                };
                Paths: {
                    $visitor: typeof PathsVisitor;
                };
                PathItem: {
                    $visitor: typeof PathItemVisitor;
                    fixedFields: {
                        $ref: typeof PathItem$RefVisitor;
                        get: {
                            $ref: string;
                        };
                        put: {
                            $ref: string;
                        };
                        post: {
                            $ref: string;
                        };
                        delete: {
                            $ref: string;
                        };
                        options: {
                            $ref: string;
                        };
                        head: {
                            $ref: string;
                        };
                        patch: {
                            $ref: string;
                        };
                        parameters: typeof PathItemParametersVisitor;
                    };
                };
                Operation: {
                    $visitor: typeof OperationVisitor;
                    fixedFields: {
                        tags: typeof OperationTagsVisitor;
                        summary: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        externalDocs: {
                            $ref: string;
                        };
                        operationId: {
                            $ref: string;
                        };
                        consumes: typeof OperationConsumesVisitor;
                        produces: typeof OperationProducesVisitor;
                        parameters: typeof OperationParametersVisitor;
                        responses: {
                            $ref: string;
                        };
                        schemes: typeof OperationSchemesVisitor;
                        deprecated: {
                            $ref: string;
                        };
                        security: typeof OperationSecurityVisitor;
                    };
                };
                ExternalDocumentation: {
                    $visitor: typeof ExternalDocumentationVisitor;
                    fixedFields: {
                        description: {
                            $ref: string;
                        };
                        url: {
                            $ref: string;
                        };
                    };
                };
                Parameter: {
                    $visitor: typeof ParameterVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        in: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        required: RequiredVisitor;
                        schema: typeof SchemaOrJSONReferenceVisitor;
                        type: TypeVisitor;
                        format: {
                            $ref: string;
                        };
                        items: {
                            $ref: string;
                        };
                        collectionFormat: {
                            $ref: string;
                        };
                        default: {
                            $ref: string;
                        };
                        maximum: {
                            $ref: string;
                        };
                        exclusiveMaximum: {
                            $ref: string;
                        };
                        minimum: {
                            $ref: string;
                        };
                        exclusiveMinimum: {
                            $ref: string;
                        };
                        maxLength: {
                            $ref: string;
                        };
                        minLength: {
                            $ref: string;
                        };
                        pattern: {
                            $ref: string;
                        };
                        maxItems: {
                            $ref: string;
                        };
                        minItems: {
                            $ref: string;
                        };
                        uniqueItems: {
                            $ref: string;
                        };
                        enum: EnumVisitor;
                        multipleOf: {
                            $ref: string;
                        };
                    };
                };
                Items: {
                    $visitor: typeof ItemsVisitor;
                    fixedFields: {
                        type: TypeVisitor;
                        format: {
                            $ref: string;
                        };
                        items: {
                            $ref: string;
                        };
                        collectionFormat: {
                            $ref: string;
                        };
                        default: {
                            $ref: string;
                        };
                        maximum: {
                            $ref: string;
                        };
                        exclusiveMaximum: {
                            $ref: string;
                        };
                        minimum: {
                            $ref: string;
                        };
                        exclusiveMinimum: {
                            $ref: string;
                        };
                        maxLength: {
                            $ref: string;
                        };
                        minLength: {
                            $ref: string;
                        };
                        pattern: {
                            $ref: string;
                        };
                        maxItems: {
                            $ref: string;
                        };
                        minItems: {
                            $ref: string;
                        };
                        uniqueItems: {
                            $ref: string;
                        };
                        enum: EnumVisitor;
                        multipleOf: {
                            $ref: string;
                        };
                    };
                };
                Responses: {
                    $visitor: typeof ResponsesVisitor;
                    fixedFields: {
                        default: typeof ResponsesDefaultVisitor;
                    };
                };
                Response: {
                    $visitor: typeof ResponseVisitor;
                    fixedFields: {
                        description: {
                            $ref: string;
                        };
                        schema: typeof SchemaOrJSONReferenceVisitor;
                        headers: {
                            $ref: string;
                        };
                        examples: {
                            $ref: string;
                        };
                    };
                };
                Headers: {
                    $visitor: typeof HeadersVisitor;
                };
                Example: {
                    $visitor: typeof ExampleVisitor;
                };
                Header: {
                    $visitor: typeof HeaderVisitor;
                    fixedFields: {
                        description: {
                            $ref: string;
                        };
                        type: TypeVisitor;
                        format: {
                            $ref: string;
                        };
                        items: {
                            $ref: string;
                        };
                        collectionFormat: {
                            $ref: string;
                        };
                        default: {
                            $ref: string;
                        };
                        maximum: {
                            $ref: string;
                        };
                        exclusiveMaximum: {
                            $ref: string;
                        };
                        minimum: {
                            $ref: string;
                        };
                        exclusiveMinimum: {
                            $ref: string;
                        };
                        maxLength: {
                            $ref: string;
                        };
                        minLength: {
                            $ref: string;
                        };
                        pattern: {
                            $ref: string;
                        };
                        maxItems: {
                            $ref: string;
                        };
                        minItems: {
                            $ref: string;
                        };
                        uniqueItems: {
                            $ref: string;
                        };
                        enum: EnumVisitor;
                        multipleOf: {
                            $ref: string;
                        };
                    };
                };
                Tag: {
                    $visitor: typeof TagVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        externalDocs: {
                            $ref: string;
                        };
                    };
                };
                Reference: {
                    $visitor: typeof ReferenceVisitor;
                    fixedFields: {
                        $ref: typeof Reference$RefVisitor;
                    };
                };
                Schema: {
                    $visitor: typeof SchemaVisitor;
                    fixedFields: {
                        format: {
                            $ref: string;
                        };
                        title: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        default: {
                            $ref: string;
                        };
                        multipleOf: {
                            $ref: string;
                        };
                        maximum: {
                            $ref: string;
                        };
                        exclusiveMaximum: {
                            $ref: string;
                        };
                        minimum: {
                            $ref: string;
                        };
                        exclusiveMinimum: {
                            $ref: string;
                        };
                        maxLength: {
                            $ref: string;
                        };
                        minLength: {
                            $ref: string;
                        };
                        pattern: {
                            $ref: string;
                        };
                        maxItems: {
                            $ref: string;
                        };
                        minItems: {
                            $ref: string;
                        };
                        uniqueItems: {
                            $ref: string;
                        };
                        maxProperties: {
                            $ref: string;
                        };
                        minProperties: {
                            $ref: string;
                        };
                        required: RequiredVisitor;
                        enum: EnumVisitor;
                        type: TypeVisitor;
                        readOnly: {
                            $ref: string;
                        };
                        items: typeof SchemaItemsVisitor;
                        allOf: typeof SchemaAllOfVisitor;
                        properties: typeof SchemaPropertiesVisitor;
                        additionalProperties: typeof SchemaOrJSONReferenceVisitor;
                        discriminator: {
                            $ref: string;
                        };
                        xml: {
                            $ref: string;
                        };
                        externalDocs: {
                            $ref: string;
                        };
                        example: {
                            $ref: string;
                        };
                    };
                };
                XML: {
                    $visitor: typeof XmlVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        namespace: {
                            $ref: string;
                        };
                        prefix: {
                            $ref: string;
                        };
                        attribute: {
                            $ref: string;
                        };
                        wrapped: {
                            $ref: string;
                        };
                    };
                };
                Definitions: {
                    $visitor: typeof DefinitionsVisitor;
                };
                ParametersDefinitions: {
                    $visitor: typeof ParametersDefinitionsVisitor;
                };
                ResponsesDefinitions: {
                    $visitor: typeof ResponsesDefinitionsVisitor;
                };
                SecurityDefinitions: {
                    $visitor: typeof SecurityDefinitionsVisitor;
                };
                SecurityScheme: {
                    $visitor: typeof SecuritySchemeVisitor;
                    fixedFields: {
                        type: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        name: {
                            $ref: string;
                        };
                        in: {
                            $ref: string;
                        };
                        flow: {
                            $ref: string;
                        };
                        authorizationUrl: {
                            $ref: string;
                        };
                        token: {
                            $ref: string;
                        };
                        scopes: {
                            $ref: string;
                        };
                    };
                };
                Scopes: {
                    $visitor: typeof ScopesVisitor;
                };
                SecurityRequirement: {
                    $visitor: typeof SecurityRequirementVisitor;
                };
            };
            extension: {
                $visitor: typeof SpecificationExtensionVisitor;
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
    constructor({ specObj, ...rest }: SpecificationVisitorOptions);
    retrievePassingOptions(): Pick<this, keyof this>;
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
export declare interface SpecificationVisitorOptions extends VisitorOptions {
    readonly specObj: typeof specificationObj;
}

/**
 * @public
 */
export declare type SpecPath<T = string[]> = (element: unknown) => T;

/**
 * @public
 */
export declare class SwaggerConsumesElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SwaggerElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get swagger(): SwaggerVersionElement | undefined;
    set swagger(swagger: SwaggerVersionElement | undefined);
    get info(): InfoElement | undefined;
    set info(info: InfoElement | undefined);
    get host(): StringElement | undefined;
    set host(host: StringElement | undefined);
    get basePath(): StringElement | undefined;
    set basePath(basePath: StringElement | undefined);
    get schemes(): ArrayElement | undefined;
    set schemes(schemes: ArrayElement | undefined);
    get consumes(): ArrayElement | undefined;
    set consumes(consumes: ArrayElement | undefined);
    get produces(): ArrayElement | undefined;
    set produces(produces: ArrayElement | undefined);
    get paths(): PathsElement | undefined;
    set paths(paths: PathsElement | undefined);
    get definitions(): DefinitionsElement | undefined;
    set definitions(definitions: DefinitionsElement | undefined);
    get parameters(): ParametersDefinitionsElement | undefined;
    set parameters(parameters: ParametersDefinitionsElement | undefined);
    get responses(): ResponsesDefinitionsElement | undefined;
    set responses(responses: ResponsesDefinitionsElement | undefined);
    get securityDefinitions(): SecurityDefinitionsElement | undefined;
    set securityDefinitions(securityDefinitions: SecurityDefinitionsElement | undefined);
    get security(): ArrayElement | undefined;
    set security(security: ArrayElement | undefined);
    get tags(): ArrayElement | undefined;
    set tags(tags: ArrayElement | undefined);
    get externalDocs(): ExternalDocumentationElement | undefined;
    set externalDocs(externalDocs: ExternalDocumentationElement | undefined);
}

/**
 * @public
 */
export declare class SwaggerProducesElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SwaggerSchemesElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SwaggerSecurityElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SwaggerTagsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SwaggerVersionElement extends StringElement {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SwaggerVersionVisitor extends SwaggerVisitor_base_2 {
    element: SwaggerVersionElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare interface SwaggerVersionVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SwaggerVisitor extends SwaggerVisitor_base {
    readonly element: SwaggerElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Swagger']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: SwaggerVisitorOptions);
}

declare const SwaggerVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

declare const SwaggerVisitor_base_2: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SwaggerVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

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
export declare class TagsVisitor extends TagsVisitor_base {
    readonly element: SwaggerTagsElement;
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
export { VisitorOptions as BasePathVisitorOptions }
export { VisitorOptions as ConsumesVisitorOptions }
export { VisitorOptions as FallbackVisitorOptions }
export { VisitorOptions as HostVisitorOptions }
export { VisitorOptions as InfoVersionVisitorOptions }
export { VisitorOptions as OperationConsumesVisitorOptions }
export { VisitorOptions as OperationProducesVisitorOptions }
export { VisitorOptions as OperationSchemesVisitorOptions }
export { VisitorOptions as OperationTagsVisitorOptions }
export { VisitorOptions as PathItem$RefVisitorOptions }
export { VisitorOptions as ProducesVisitorOptions }
export { VisitorOptions as Reference$RefVisitorOptions }
export { VisitorOptions as SchemesVisitorOptions }
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
