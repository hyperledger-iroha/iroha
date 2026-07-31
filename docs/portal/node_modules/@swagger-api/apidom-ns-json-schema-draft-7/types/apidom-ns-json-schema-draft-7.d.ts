import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { AllOfVisitor as AllOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AllOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { AlternatingVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { AlternatingVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { AnyOfVisitor as AnyOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { DefinitionsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { DefinitionsVisitor as DefinitionsVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DefinitionsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { DependenciesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { DependenciesVisitor as DependenciesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { FallbackVisitor as FallbackVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { FixedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { FixedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { getNodeType } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isJSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { isJSONReferenceLikeElement } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { isLinkElement } from '@swagger-api/apidom-core';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isRefElement } from '@swagger-api/apidom-core';
import { isStringElement } from '@swagger-api/apidom-core';
import { ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ItemsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { JSONReference$RefVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { JSONReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaDraft4ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { JSONSchemaElement as JSONSchemaElement_2 } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { JSONSchemaVisitor as JSONSchemaVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaVisitor as JSONSchemaVisitor_3 } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { JSONSchemaVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { LinkDescriptionElement as LinkDescriptionElement_2 } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_3 } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { LinkDescriptionVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { LinksVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MapVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { MapVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { MediaElement } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MediaVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { OneOfVisitor as OneOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { OneOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { ParentSchemaAwareVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { PatternedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { PatternedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { PatternPropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { PatternPropertiesVisitor as PatternPropertiesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { PropertiesVisitor as PropertiesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { SchemaOrReferenceVisitor as SchemaOrReferenceVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { specificationObj as specificationObj_2 } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { SpecificationVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { SpecificationVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { SpecPath } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { StringElement } from '@swagger-api/apidom-core';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Visitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import { VisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-6';

export { AllOfVisitor }

export { AllOfVisitorOptions }

export { AlternatingVisitor }

export { AlternatingVisitorOptions }

export { AnyOfVisitor }

export { AnyOfVisitorOptions }

/**
 * @public
 */
export declare const createRefractor: (specPath: string[]) => (value: unknown, options?: {}) => Element_2;

export { DefinitionsVisitor }

export { DefinitionsVisitorOptions }

export { DependenciesVisitor }

export { DependenciesVisitorOptions }

export { FallbackVisitor }

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

export { isJSONReferenceElement }

export { isJSONReferenceLikeElement }

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

export { ItemsVisitorOptions }

export { JSONReferenceElement }

export { JSONSchemaDraft4ItemsVisitor }

/**
 * @public
 */
declare const jsonSchemaDraft7: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default jsonSchemaDraft7;

/**
 * @public
 */
export declare class JSONSchemaDraft7MediaTypes extends MediaTypes<string> {
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
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-01
     */
    get $comment(): StringElement | undefined;
    set $comment($comment: StringElement | undefined);
    /**
     * Validation vocabulary
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01
     */
    /**
     * Validation keywords for arrays
     */
    get items(): this | BooleanElement | JSONReferenceElement | ArrayElement | undefined;
    set items(items: this | BooleanElement | JSONReferenceElement | ArrayElement | undefined);
    /**
     * Keywords for Applying Subschemas Conditionally
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.6
     */
    get if(): this | BooleanElement | JSONReferenceElement | undefined;
    set if(ifValue: this | BooleanElement | JSONReferenceElement | undefined);
    get then(): this | BooleanElement | JSONReferenceElement | undefined;
    set then(then: this | BooleanElement | JSONReferenceElement | undefined);
    get else(): this | BooleanElement | JSONReferenceElement | undefined;
    set else(elseValue: this | BooleanElement | JSONReferenceElement | undefined);
    /**
     * Keywords for Applying Subschemas With Boolean Logic
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.7
     */
    get not(): this | BooleanElement | JSONReferenceElement | undefined;
    set not(not: this | BooleanElement | JSONReferenceElement | undefined);
    /**
     * String-Encoding Non-JSON Data
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-00#section-8
     */
    get contentEncoding(): StringElement | undefined;
    set contentEncoding(contentEncoding: StringElement | undefined);
    get contentMediaType(): StringElement | undefined;
    set contentMediaType(contentMediaType: StringElement | undefined);
    get media(): MediaElement | undefined;
    set media(media: MediaElement | undefined);
    /**
     * Schema annotations
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-10
     */
    get writeOnly(): BooleanElement | undefined;
    set writeOnly(writeOnly: BooleanElement | undefined);
}

/**
 * @public
 */
export declare class JSONSchemaVisitor extends JSONSchemaVisitor_3 {
    element: JSONSchemaElement;
    constructor(options: JSONSchemaVisitorOptions);
    get defaultDialectIdentifier(): string;
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
    JSONSchemaDraft7Element: string[];
    JSONReferenceElement: string[];
    LinkDescriptionElement: string[];
};

/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-01#section-6
 * @public
 */
export declare class LinkDescriptionElement extends LinkDescriptionElement_2 {
    /**
     * Link context.
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-hyperschema-01#section-6.1
     */
    get anchor(): StringElement | undefined;
    set anchor(anchor: StringElement | undefined);
    get anchorPointer(): StringElement | undefined;
    set anchorPointer(anchorPointer: StringElement | undefined);
    /**
     * Adjusting URI Template Resolution.
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-hyperschema-01#section-6.4
     */
    get templatePointers(): ObjectElement | undefined;
    set templatePointers(templatePointers: ObjectElement | undefined);
    get templateRequired(): ArrayElement | undefined;
    set templateRequired(templateRequired: ArrayElement | undefined);
    /**
     * Link Target Attributes.
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-hyperschema-01#section-6.5
     */
    get targetSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined;
    set targetSchema(targetSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined);
    get mediaType(): StringElement | undefined;
    set mediaType(mediaType: StringElement | undefined);
    get targetMediaType(): StringElement | undefined;
    set targetMediaType(targetMediaType: StringElement | undefined);
    get targetHints(): ObjectElement | undefined;
    set targetHints(targetHints: ObjectElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get $comment(): StringElement | undefined;
    set $comment($comment: StringElement | undefined);
    /**
     *  Link Input.
     *
     *  URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-hyperschema-01#section-6.6
     */
    get hrefSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined;
    set hrefSchema(hrefSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined);
    get headerSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined;
    set headerSchema(headerSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined);
    /**
     *  Submitting Data for Processing.
     *
     *  URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-hyperschema-01#section-6.6.4
     */
    get submissionSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined;
    set submissionSchema(submissionSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined);
    get submissionEncType(): StringElement | undefined;
    set submissionEncType(submissionEncType: StringElement | undefined);
    get submissionMediaType(): StringElement | undefined;
    set submissionMediaType(submissionMediaType: StringElement | undefined);
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
export declare const mediaTypes: JSONSchemaDraft7MediaTypes;

export { OneOfVisitor }

export { OneOfVisitorOptions }

export { ParentSchemaAwareVisitor }

export { ParentSchemaAwareVisitorOptions }

export { PatternedFieldsVisitor }

export { PatternedFieldsVisitorOptions }

export { PatternPropertiesVisitor }

export { PatternPropertiesVisitorOptions }

export { PropertiesVisitor }

export { PropertiesVisitorOptions }

/**
 * @public
 */
export declare const refract: <T extends Element_2>(value: unknown, { specPath, plugins, specificationObj, }?: {
    specPath?: string[] | undefined;
    plugins?: never[] | undefined;
    specificationObj?: {
        visitors: {
            value: FallbackVisitor_2;
            JSONSchemaOrJSONReferenceVisitor: SchemaOrReferenceVisitor_2;
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
                            additionalItems: SchemaOrReferenceVisitor_2;
                            items: ItemsVisitor;
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
                            additionalProperties: SchemaOrReferenceVisitor_2;
                            patternProperties: PatternPropertiesVisitor_2;
                            dependencies: DependenciesVisitor_2;
                            enum: EnumVisitor;
                            type: TypeVisitor;
                            allOf: AllOfVisitor_2;
                            anyOf: AnyOfVisitor_2;
                            oneOf: OneOfVisitor_2;
                            not: SchemaOrReferenceVisitor_2;
                            definitions: DefinitionsVisitor_2;
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
                            targetSchema: SchemaOrReferenceVisitor_2;
                            mediaType: {
                                $ref: string;
                            };
                            method: {
                                $ref: string;
                            };
                            encType: {
                                $ref: string;
                            };
                            schema: SchemaOrReferenceVisitor_2;
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

export { SchemaOrReferenceVisitor }

export { SchemaOrReferenceVisitorOptions }

export declare const specificationObj: typeof specificationObj_2;

export { SpecificationVisitor }

export { SpecificationVisitorOptions }

export { SpecPath }

export { Visitor }

export { VisitorOptions }

export { }
