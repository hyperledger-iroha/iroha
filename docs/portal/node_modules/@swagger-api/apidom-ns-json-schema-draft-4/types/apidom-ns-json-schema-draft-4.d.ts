import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { Class } from 'ts-mixer/dist/types/types.js';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isLinkElement } from '@swagger-api/apidom-core';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isRefElement } from '@swagger-api/apidom-core';
import { isStringElement } from '@swagger-api/apidom-core';
import { MediaTypes } from '@swagger-api/apidom-core';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { NumberElement } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
export declare class AllOfVisitor extends AllOfVisitor_base {
    readonly element: ArrayElement;
    constructor(options: AllOfVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const AllOfVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface AllOfVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

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
declare interface AlternatingVisitorOptions extends SpecificationVisitorOptions {
    readonly alternator: Alternator[];
}
export { AlternatingVisitorOptions }
export { AlternatingVisitorOptions as SchemaOrReferenceVisitorOptions }

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
export declare class AnyOfVisitor extends AnyOfVisitor_base {
    readonly element: ArrayElement;
    constructor(options: AnyOfVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const AnyOfVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface AnyOfVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare const createRefractor: (specPath: string[]) => (value: unknown, options?: {}) => Element_2;

/**
 * @public
 */
export declare class DefinitionsVisitor extends DefinitionsVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'JSONReference'
    ] | ['document', 'objects', 'JSONSchema']>;
    constructor(options: DefinitionsVisitorOptions);
}

declare const DefinitionsVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface DefinitionsVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class DependenciesVisitor extends DependenciesVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'JSONReference'
    ] | ['document', 'objects', 'JSONSchema']>;
    constructor(options: DependenciesVisitorOptions);
}

declare const DependenciesVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface DependenciesVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class EnumVisitor extends FallbackVisitor {
    readonly element: ArrayElement;
    ArrayElement(arrayElement: ArrayElement): {};
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
    constructor({ specPath, ignoredFields, ...rest }: FixedFieldsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface FixedFieldsVisitorOptions extends SpecificationVisitorOptions {
    readonly specPath: SpecPath;
    readonly ignoredFields?: string[];
}

/**
 * @public
 */
export declare type Format = 'generic' | 'json' | 'yaml';

/**
 * @public
 */
export declare const getNodeType: <T extends Element_2>(element: T) => string | undefined;

export { isArrayElement }

export { isBooleanElement }

export { isElement }

/**
 * @public
 */
export declare const isJSONReferenceElement: ElementPredicate<JSONReferenceElement>;

/**
 * @public
 */
export declare const isJSONReferenceLikeElement: (element: unknown) => element is JSONReferenceLikeElement;

/**
 * @public
 */
export declare const isJSONSchemaElement: ElementPredicate<JSONSchemaElement>;

/**
 * @public
 */
export declare const isLinkDescriptionElement: ElementPredicate<LinkDescriptionElement>;

export { isLinkElement }

/**
 * @public
 */
export declare const isMediaElement: ElementPredicate<MediaElement>;

export { isMemberElement }

export { isNullElement }

export { isNumberElement }

export { isObjectElement }

export { isRefElement }

export { isStringElement }

/**
 * @public
 */
export declare class ItemsVisitor extends ItemsVisitor_base {
    element: ArrayElement | ObjectElement;
    ObjectElement(objectElement: ObjectElement): {};
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const ItemsVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ItemsVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class JSONReference$RefVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * In Draft 4, $ref behaves a little differently. When an object contains a $ref property,
 * the object is considered a reference, not a schema. Therefore, any other properties you put in that object will
 * not be treated as JSON Schema keywords and will be ignored by the validator.
 * $ref can only be used where a schema is expected.
 *
 * URI: https://json-schema.org/understanding-json-schema/structuring.html?highlight=ref#id18
 * @public
 */
export declare class JSONReferenceElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get $ref(): StringElement | undefined;
    set $ref($ref: StringElement | undefined);
}

/**
 * @public
 */
export declare interface JSONReferenceLikeElement extends ObjectElement {
    hasKey: (value: '$ref') => true;
}

/**
 * @public
 */
export declare class JSONReferenceVisitor extends JSONReferenceVisitor_base {
    readonly element: JSONReferenceElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONReference']>;
    constructor(options: JSONReferenceVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const JSONReferenceVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface JSONReferenceVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
declare const jsonSchemaDraft4: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default jsonSchemaDraft4;

/**
 * @public
 */
export declare class JSONSchemaDraft4MediaTypes extends MediaTypes<string> {
    filterByFormat(format?: Format): string[];
    findBy(version?: string, format?: Format): string;
    latest(format?: Format): string;
}

/**
 * @public
 */
export declare class JSONSchemaElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    /**
     * Core vocabulary
     *
     * URI: https://tools.ietf.org/html/draft-wright-json-schema-00
     */
    get idProp(): StringElement | undefined;
    set idProp(idProp: StringElement | undefined);
    get $schema(): StringElement | undefined;
    set $schema($schema: StringElement | undefined);
    /**
     * Validation vocabulary
     *
     * URI: https://tools.ietf.org/html/draft-wright-json-schema-validation-00
     */
    /**
     *  Validation keywords for numeric instances (number and integer)
     */
    get multipleOf(): NumberElement | undefined;
    set multipleOf(multipleOf: NumberElement | undefined);
    get maximum(): NumberElement | undefined;
    set maximum(maximum: NumberElement | undefined);
    get exclusiveMaximum(): BooleanElement | undefined;
    set exclusiveMaximum(exclusiveMaximum: BooleanElement | undefined);
    get minimum(): NumberElement | undefined;
    set minimum(minimum: NumberElement | undefined);
    get exclusiveMinimum(): BooleanElement | undefined;
    set exclusiveMinimum(exclusiveMinimum: BooleanElement | undefined);
    /**
     * Validation keywords for strings
     */
    get maxLength(): NumberElement | undefined;
    set maxLength(maxLength: NumberElement | undefined);
    get minLength(): NumberElement | undefined;
    set minLength(minLength: NumberElement | undefined);
    get pattern(): StringElement | undefined;
    set pattern(pattern: StringElement | undefined);
    /**
     * Validation keywords for arrays
     */
    get additionalItems(): this | JSONReferenceElement | BooleanElement | undefined;
    set additionalItems(additionalItems: this | JSONReferenceElement | BooleanElement | undefined);
    get items(): this | JSONReferenceElement | ArrayElement | undefined;
    set items(items: this | JSONReferenceElement | ArrayElement | undefined);
    get maxItems(): NumberElement | undefined;
    set maxItems(maxItems: NumberElement | undefined);
    get minItems(): NumberElement | undefined;
    set minItems(minItems: NumberElement | undefined);
    get uniqueItems(): BooleanElement | undefined;
    set uniqueItems(uniqueItems: BooleanElement | undefined);
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
    get enum(): ArrayElement | undefined;
    set enum(enumValue: ArrayElement | undefined);
    get type(): ArrayElement | StringElement | undefined;
    set type(type: ArrayElement | StringElement | undefined);
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
    get default(): Element_2 | undefined;
    set default(defaultValue: Element_2 | undefined);
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
}

/**
 * @public
 */
export declare class JSONSchemaVisitor extends JSONSchemaVisitor_base {
    element: JSONSchemaElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONSchema']>;
    constructor(options: JSONSchemaVisitorOptions);
    get defaultDialectIdentifier(): string;
    ObjectElement(objectElement: ObjectElement): {};
    handleDialectIdentifier(objectElement: ObjectElement): void;
    handleSchemaIdentifier(objectElement: ObjectElement, identifierKeyword?: string): void;
}

declare const JSONSchemaVisitor_base: Class<any[], FixedFieldsVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface JSONSchemaVisitorOptions extends FixedFieldsVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
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
    JSONSchemaDraft4Element: string[];
    JSONReferenceElement: string[];
    MediaElement: string[];
    LinkDescriptionElement: string[];
};

/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00#section-5
 * @public
 */
export declare class LinkDescriptionElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get href(): StringElement | undefined;
    set href(href: StringElement | undefined);
    get rel(): StringElement | undefined;
    set rel(rel: StringElement | undefined);
    get title(): StringElement | undefined;
    set title(title: StringElement | undefined);
    get targetSchema(): JSONSchemaElement | JSONReferenceElement | undefined;
    set targetSchema(targetSchema: JSONSchemaElement | JSONReferenceElement | undefined);
    get mediaType(): StringElement | undefined;
    set mediaType(mediaType: StringElement | undefined);
    get method(): StringElement | undefined;
    set method(method: StringElement | undefined);
    get encType(): StringElement | undefined;
    set encType(encType: StringElement | undefined);
    get schema(): JSONSchemaElement | JSONReferenceElement | undefined;
    set schema(schema: JSONSchemaElement | JSONReferenceElement | undefined);
}

/**
 * @public
 */
export declare class LinkDescriptionVisitor extends LinkDescriptionVisitor_base {
    readonly element: LinkDescriptionElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'LinkDescription']>;
    constructor(options: LinkDescriptionVisitorOptions);
}

declare const LinkDescriptionVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface LinkDescriptionVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class LinksVisitor extends LinksVisitor_base {
    readonly element: ArrayElement;
    constructor(options: LinksVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const LinksVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface LinksVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
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
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00#section-4.3
 * @public
 */
export declare class MediaElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get binaryEncoding(): StringElement | undefined;
    set binaryEncoding(binaryEncoding: StringElement | undefined);
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
}

/**
 * @public
 */
export declare const mediaTypes: JSONSchemaDraft4MediaTypes;

/**
 * @public
 */
export declare class MediaVisitor extends MediaVisitor_base {
    readonly element: MediaElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Media']>;
    constructor(options: MediaVisitorOptions);
}

declare const MediaVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface MediaVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class OneOfVisitor extends OneOfVisitor_base {
    readonly element: ArrayElement;
    constructor(options: OneOfVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const OneOfVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface OneOfVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ParentSchemaAwareVisitor {
    parent: Element_2;
    constructor({ parent }: ParentSchemaAwareVisitorOptions);
}

/**
 * @public
 */
export declare interface ParentSchemaAwareVisitorOptions {
    readonly parent: Element_2;
}

/**
 * @public
 */
export declare class PatternedFieldsVisitor extends SpecificationVisitor {
    protected specPath: SpecPath;
    protected ignoredFields: string[];
    protected fieldPatternPredicate: (value: unknown) => boolean;
    constructor({ specPath, ignoredFields, fieldPatternPredicate, ...rest }: PatternedFieldsVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare interface PatternedFieldsVisitorOptions extends SpecificationVisitorOptions {
    readonly specPath: SpecPath;
    readonly ignoredFields?: string[];
    readonly fieldPatternPredicate?: (...args: unknown[]) => boolean;
}

/**
 * @public
 */
export declare class PatternPropertiesVisitor extends PatternPropertiesVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'JSONReference'
    ] | ['document', 'objects', 'JSONSchema']>;
    constructor(options: PatternPropertiesVisitorOptions);
}

declare const PatternPropertiesVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PatternPropertiesVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class PropertiesVisitor extends PropertiesVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<[
    'document',
    'objects',
    'JSONReference'
    ] | ['document', 'objects', 'JSONSchema']>;
    constructor(options: PropertiesVisitorOptions);
}

declare const PropertiesVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PropertiesVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare const refract: <T extends Element_2>(value: unknown, { specPath, plugins, specificationObj, }?: {
    specPath?: string[];
    plugins?: any[];
    specificationObj?: object;
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
export declare class RequiredVisitor extends FallbackVisitor {
    readonly element: ArrayElement;
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare class SchemaOrReferenceVisitor extends AlternatingVisitor {
    constructor(options: AlternatingVisitorOptions);
}

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
    visitors: {
        value: typeof FallbackVisitor;
        JSONSchemaOrJSONReferenceVisitor: typeof SchemaOrReferenceVisitor;
        document: {
            objects: {
                JSONSchema: {
                    $visitor: typeof JSONSchemaVisitor;
                    fixedFields: {
                        id: {
                            $ref: string;
                        };
                        $schema: {
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
                        additionalItems: typeof SchemaOrReferenceVisitor;
                        items: typeof ItemsVisitor;
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
                        required: typeof RequiredVisitor;
                        properties: typeof PropertiesVisitor;
                        additionalProperties: typeof SchemaOrReferenceVisitor;
                        patternProperties: typeof PatternPropertiesVisitor;
                        dependencies: typeof DependenciesVisitor;
                        enum: typeof EnumVisitor;
                        type: typeof TypeVisitor;
                        allOf: typeof AllOfVisitor;
                        anyOf: typeof AnyOfVisitor;
                        oneOf: typeof OneOfVisitor;
                        not: typeof SchemaOrReferenceVisitor;
                        definitions: typeof DefinitionsVisitor;
                        title: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        default: {
                            $ref: string;
                        };
                        format: {
                            $ref: string;
                        };
                        base: {
                            $ref: string;
                        };
                        links: typeof LinksVisitor;
                        media: {
                            $ref: string;
                        };
                        readOnly: {
                            $ref: string;
                        };
                    };
                };
                JSONReference: {
                    $visitor: typeof JSONReferenceVisitor;
                    fixedFields: {
                        $ref: typeof JSONReference$RefVisitor;
                    };
                };
                Media: {
                    $visitor: typeof MediaVisitor;
                    fixedFields: {
                        binaryEncoding: {
                            $ref: string;
                        };
                        type: {
                            $ref: string;
                        };
                    };
                };
                LinkDescription: {
                    $visitor: typeof LinkDescriptionVisitor;
                    fixedFields: {
                        href: {
                            $ref: string;
                        };
                        rel: {
                            $ref: string;
                        };
                        title: {
                            $ref: string;
                        };
                        targetSchema: typeof SchemaOrReferenceVisitor;
                        mediaType: {
                            $ref: string;
                        };
                        method: {
                            $ref: string;
                        };
                        encType: {
                            $ref: string;
                        };
                        schema: typeof SchemaOrReferenceVisitor;
                    };
                };
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
export declare class TypeVisitor extends FallbackVisitor {
    readonly element: StringElement | ArrayElement;
    StringElement(stringElement: StringElement): {};
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare class Visitor {
    element: Element_2;
    constructor(options: VisitorOptions);
    copyMetaAndAttributes(from: Element_2, to: Element_2): void;
}

/**
 * @public
 */
declare interface VisitorOptions {
}
export { VisitorOptions as EnumVisitorOptions }
export { VisitorOptions as FallbackVisitorOptions }
export { VisitorOptions as JSONReference$RefVisitorOptions }
export { VisitorOptions as RequiredVisitorOptions }
export { VisitorOptions as TypeVisitorOptions }
export { VisitorOptions }

export { }
