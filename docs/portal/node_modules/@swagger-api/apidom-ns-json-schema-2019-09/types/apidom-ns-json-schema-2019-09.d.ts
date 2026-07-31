import { AllOfVisitor as AllOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AlternatingVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { AlternatingVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { AnyOfVisitor as AnyOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { Class } from 'ts-mixer/dist/types/types.js';
import { DefinitionsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { FallbackVisitor as FallbackVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { FixedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { FixedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { getNodeType } from '@swagger-api/apidom-ns-json-schema-draft-7';
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
import { ItemsVisitor as ItemsVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReference$RefVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaElement as JSONSchemaElement_2 } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { JSONSchemaVisitor as JSONSchemaVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaVisitor as JSONSchemaVisitor_3 } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { JSONSchemaVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { LinkDescriptionElement as LinkDescriptionElement_2 } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_3 } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { LinkDescriptionVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { LinksVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MapVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { MapVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MediaVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { NumberElement } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { OneOfVisitor as OneOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { ParentSchemaAwareVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { PatternedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { PatternedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { PatternPropertiesVisitor as PatternPropertiesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitor as PropertiesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { specificationObj as specificationObj_2 } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { SpecificationVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { SpecificationVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { SpecPath } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { StringElement } from '@swagger-api/apidom-core';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Visitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { VisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-7';

/**
 * @public
 */
export declare class $defsVisitor extends $defsVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONSchema']>;
    constructor(options: $defsVisitorOptions);
}

declare const $defsVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface $defsVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
export declare class $refVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class $vocabularyVisitor extends FallbackVisitor {
    readonly element: ObjectElement;
    ObjectElement(objectElement: ObjectElement): {};
}

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
export declare interface AllOfVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

export { AlternatingVisitor }

export { AlternatingVisitorOptions }

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
export declare interface AnyOfVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
export declare const createRefractor: (specPath: string[]) => (value: unknown, options?: {}) => Element_2;

/**
 * @public
 */
export declare class DependentRequiredVisitor extends FallbackVisitor {
    readonly element: ObjectElement;
    ObjectElement(objectElement: ObjectElement): {};
}

/**
 * @public
 */
export declare class DependentSchemasVisitor extends DependentSchemasVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONSchema']>;
    constructor(options: DependentSchemasVisitorOptions);
}

declare const DependentSchemasVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface DependentSchemasVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

export { FallbackVisitor }

export { FallbackVisitorOptions as $refVisitorOptions }
export { FallbackVisitorOptions as $vocabularyVisitorOptions }
export { FallbackVisitorOptions as DependentRequiredVisitorOptions }
export { FallbackVisitorOptions }

export { FixedFieldsVisitor }

export { FixedFieldsVisitorOptions }

/**
 * @public
 */
export declare type Format = 'generic' | 'json' | 'yaml';

export { getNodeType }

export { isArrayElement }

export { isBooleanElement }

export { isElement }

/**
 * @public
 */
export declare const isJSONSchemaElement: ElementPredicate<JSONSchemaElement>;

/**
 * @public
 */
export declare const isLinkDescriptionElement: ElementPredicate<LinkDescriptionElement>;

export { isLinkElement }

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
    element: ObjectElement | ArrayElement;
    ObjectElement(objectElement: ObjectElement): {};
    ArrayElement(arrayElement: ArrayElement): {};
    BooleanElement(booleanElement: BooleanElement): {};
}

declare const ItemsVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ItemsVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
declare const jsonSchema201909: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default jsonSchema201909;

/**
 * @public
 */
export declare class JSONSchema201909MediaTypes extends MediaTypes<string> {
    filterByFormat(format?: Format): string[];
    findBy(version?: string, format?: Format): string;
    latest(format?: Format): string;
}

/**
 * @public
 */
export declare class JSONSchemaElement extends JSONSchemaElement_2 {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    /**
     * Core vocabulary
     *
     * URI: https://json-schema.org/draft/2019-09/vocab/core
     */
    get $vocabulary(): ObjectElement | undefined;
    set $vocabulary($vocabulary: ObjectElement | undefined);
    get $anchor(): StringElement | undefined;
    set $anchor($anchor: StringElement | undefined);
    get $recursiveAnchor(): BooleanElement | undefined;
    set $recursiveAnchor($recursiveAnchor: BooleanElement | undefined);
    get $recursiveRef(): StringElement | undefined;
    set $recursiveRef($recursiveRef: StringElement | undefined);
    get $ref(): StringElement | undefined;
    set $ref($ref: StringElement | undefined);
    get $defs(): ObjectElement | undefined;
    set $defs($defs: ObjectElement | undefined);
    get definitions(): ObjectElement | undefined;
    set definitions(definitions: ObjectElement | undefined);
    /**
     * Applicator vocabulary
     *
     * URI: https://json-schema.org/draft/2019-09/vocab/applicator
     */
    get not(): this | BooleanElement | undefined;
    set not(not: this | BooleanElement | undefined);
    get if(): this | BooleanElement | undefined;
    set if(ifSchema: this | BooleanElement | undefined);
    get then(): this | BooleanElement | undefined;
    set then(thenSchema: this | BooleanElement | undefined);
    get else(): this | BooleanElement | undefined;
    set else(elseSchema: this | BooleanElement | undefined);
    get dependentSchemas(): ObjectElement | undefined;
    set dependentSchemas(dependentSchemas: ObjectElement | undefined);
    get dependencies(): ObjectElement | undefined;
    set dependencies(dependencies: ObjectElement | undefined);
    get items(): this | BooleanElement | ArrayElement | undefined;
    set items(items: this | BooleanElement | ArrayElement | undefined);
    get containsProp(): this | BooleanElement | undefined;
    set containsProp(containsProp: this | BooleanElement | undefined);
    get additionalProperties(): this | BooleanElement | undefined;
    set additionalProperties(additionalProperties: this | BooleanElement | undefined);
    get additionalItems(): this | BooleanElement | undefined;
    set additionalItems(additionalItems: this | BooleanElement | undefined);
    get propertyNames(): this | BooleanElement | undefined;
    set propertyNames(propertyNames: this | BooleanElement | undefined);
    get unevaluatedItems(): this | BooleanElement | undefined;
    set unevaluatedItems(unevaluatedItems: this | BooleanElement | undefined);
    get unevaluatedProperties(): this | BooleanElement | undefined;
    set unevaluatedProperties(unevaluatedProperties: this | BooleanElement | undefined);
    /**
     * Validation vocabulary
     *
     * URI: https://json-schema.org/draft/2019-09/json-schema-validation#rfc.section.6
     */
    /**
     * Validation Keywords for Arrays
     *
     * URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-validation-02#rfc.section.6.4
     */
    get maxContains(): NumberElement | undefined;
    set maxContains(maxContains: NumberElement | undefined);
    get minContains(): NumberElement | undefined;
    set minContains(minContains: NumberElement | undefined);
    /**
     * Validation Keywords for Objects
     *
     * URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-validation-02#rfc.section.6.5
     */
    get dependentRequired(): ObjectElement | undefined;
    set dependentRequired(dependentRequired: ObjectElement | undefined);
    /**
     * Vocabulary for Basic Meta-Data Annotations
     *
     * URI: https://json-schema.org/draft/2019-09/vocab/meta-data
     */
    get deprecated(): BooleanElement | undefined;
    set deprecated(deprecated: BooleanElement | undefined);
    /**
     * Vocabulary for the Contents of String-Encoded Data
     *
     * URI: https://json-schema.org/draft/2019-09/vocab/content
     */
    get contentSchema(): this | BooleanElement | undefined;
    set contentSchema(contentSchema: this | BooleanElement | undefined);
}

/**
 * @public
 */
export declare class JSONSchemaVisitor extends JSONSchemaVisitor_3 {
    element: JSONSchemaElement;
    constructor(options: JSONSchemaVisitorOptions);
    get defaultDialectIdentifier(): string;
    ObjectElement(objectElement: ObjectElement): {};
}

export { JSONSchemaVisitorOptions }

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
    JSONSchema201909Element: string[];
    LinkDescriptionElement: string[];
};

/**
 * URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#rfc.section.6
 * @public
 */
export declare class LinkDescriptionElement extends LinkDescriptionElement_2 {
    /**
     *  Link Target Attributes.
     *
     *  URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#rfc.section.6.5
     */
    get targetSchema(): JSONSchemaElement | BooleanElement | undefined;
    set targetSchema(targetSchema: JSONSchemaElement | BooleanElement | undefined);
    /**
     *  Link Input.
     *
     *  URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-hyperschema-02#input
     */
    get hrefSchema(): JSONSchemaElement | BooleanElement | undefined;
    set hrefSchema(hrefSchema: JSONSchemaElement | BooleanElement | undefined);
    get headerSchema(): JSONSchemaElement | BooleanElement | undefined;
    set headerSchema(headerSchema: JSONSchemaElement | BooleanElement | undefined);
    get submissionSchema(): JSONSchemaElement | BooleanElement | undefined;
    set submissionSchema(submissionSchema: JSONSchemaElement | BooleanElement | undefined);
}

/**
 * @public
 */
export declare class LinkDescriptionVisitor extends LinkDescriptionVisitor_3 {
    readonly element: LinkDescriptionElement;
    constructor(options: LinkDescriptionVisitorOptions);
}

export { LinkDescriptionVisitorOptions }

export { MapVisitor }

export { MapVisitorOptions }

/**
 * @public
 */
export declare const mediaTypes: JSONSchema201909MediaTypes;

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
export declare interface OneOfVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

export { ParentSchemaAwareVisitor }

export { ParentSchemaAwareVisitorOptions }

export { PatternedFieldsVisitor }

export { PatternedFieldsVisitorOptions }

/**
 * @public
 */
export declare class PatternPropertiesVisitor extends PatternPropertiesVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONSchema']>;
    constructor(options: PatternPropertiesVisitorOptions);
}

declare const PatternPropertiesVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PatternPropertiesVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
export declare class PropertiesVisitor extends PropertiesVisitor_base {
    readonly element: ObjectElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONSchema']>;
    constructor(options: PropertiesVisitorOptions);
}

declare const PropertiesVisitor_base: Class<any[], MapVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof MapVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PropertiesVisitorOptions extends MapVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

/**
 * @public
 */
export declare const refract: <T extends Element_2>(value: unknown, { specPath, plugins, specificationObj, }?: {
    specPath?: string[] | undefined;
    plugins?: never[] | undefined;
    specificationObj?: {
        visitors: {
            value: FallbackVisitor_2;
            JSONSchemaOrJSONReferenceVisitor: SchemaOrReferenceVisitor;
            document: {
                objects: {
                    JSONSchema: {
                        $visitor: JSONSchemaVisitor_2;
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
                            additionalItems: SchemaOrReferenceVisitor;
                            items: ItemsVisitor_2;
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
                            properties: PropertiesVisitor_2;
                            additionalProperties: SchemaOrReferenceVisitor;
                            patternProperties: PatternPropertiesVisitor_2;
                            dependencies: DependenciesVisitor;
                            enum: EnumVisitor;
                            type: TypeVisitor;
                            allOf: AllOfVisitor_2;
                            anyOf: AnyOfVisitor_2;
                            oneOf: OneOfVisitor_2;
                            not: SchemaOrReferenceVisitor;
                            definitions: DefinitionsVisitor;
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
                            links: LinksVisitor;
                            media: {
                                $ref: string;
                            };
                            readOnly: {
                                $ref: string;
                            };
                        };
                    };
                    JSONReference: {
                        $visitor: JSONReferenceVisitor;
                        fixedFields: {
                            $ref: JSONReference$RefVisitor;
                        };
                    };
                    Media: {
                        $visitor: MediaVisitor;
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
                        $visitor: LinkDescriptionVisitor_2;
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
                };
            };
        };
    } | undefined;
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

export declare const specificationObj: typeof specificationObj_2;

export { SpecificationVisitor }

export { SpecificationVisitorOptions }

export { SpecPath }

export { Visitor }

export { VisitorOptions }

export { }
