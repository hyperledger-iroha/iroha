import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AllOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AlternatingVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AlternatingVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { DefinitionsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DefinitionsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FixedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FixedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { getNodeType } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isJSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isJSONReferenceLikeElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isLinkElement } from '@swagger-api/apidom-core';
import { isMediaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isRefElement } from '@swagger-api/apidom-core';
import { isStringElement } from '@swagger-api/apidom-core';
import { ItemsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReference$RefVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ItemsVisitor as JSONSchemaDraft4ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaElement as JSONSchemaElement_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaVisitor as JSONSchemaVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkDescriptionElement as LinkDescriptionElement_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkDescriptionVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinksVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MapVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MapVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MediaVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { NumberElement } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { OneOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ParentSchemaAwareVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternPropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { specificationObj as specificationObj_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SpecificationVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SpecificationVisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SpecPath } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { StringElement } from '@swagger-api/apidom-core';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Visitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { VisitorOptions } from '@swagger-api/apidom-ns-json-schema-draft-4';

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

/**
 * @public
 */
export declare class ExamplesVisitor extends FallbackVisitor {
    readonly element: ArrayElement;
    ArrayElement(arrayElement: ArrayElement): {};
}

export { FallbackVisitor }

export { FallbackVisitorOptions as ExamplesVisitorOptions }
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

export { isMediaElement }

export { isMemberElement }

export { isNullElement }

export { isNumberElement }

export { isObjectElement }

export { isRefElement }

export { isStringElement }

/**
 * @public
 */
export declare class ItemsVisitor extends JSONSchemaDraft4ItemsVisitor {
    BooleanElement(booleanElement: BooleanElement): {};
}

export { ItemsVisitorOptions }

export { JSONReferenceElement }

export { JSONSchemaDraft4ItemsVisitor }

/**
 * @public
 */
declare const jsonSchemaDraft6: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default jsonSchemaDraft6;

/**
 * @public
 */
export declare class JSONSchemaDraft6MediaTypes extends MediaTypes<string> {
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
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-01
     */
    get idProp(): StringElement | undefined;
    set idProp(id: StringElement | undefined);
    get $id(): StringElement | undefined;
    set $id($id: StringElement | undefined);
    /**
     * Validation vocabulary
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-01
     */
    /**
     *  Validation keywords for numeric instances (number and integer)
     */
    get exclusiveMaximum(): NumberElement | undefined;
    set exclusiveMaximum(exclusiveMaximum: NumberElement | undefined);
    get exclusiveMinimum(): NumberElement | undefined;
    set exclusiveMinimum(exclusiveMinimum: NumberElement | undefined);
    /**
     * Validation keywords for arrays
     */
    get containsProp(): this | BooleanElement | JSONReferenceElement | undefined;
    set containsProp(contains: this | BooleanElement | JSONReferenceElement | undefined);
    get items(): this | BooleanElement | JSONReferenceElement | ArrayElement | undefined | any;
    set items(items: this | BooleanElement | JSONReferenceElement | ArrayElement | undefined | any);
    /**
     * Validation keywords for objects
     */
    get propertyNames(): this | BooleanElement | JSONReferenceElement | undefined;
    set propertyNames(propertyNames: this | BooleanElement | JSONReferenceElement | undefined);
    /**
     *  Validation keywords for any instance type
     */
    get const(): Element_2 | undefined;
    set const(constValue: Element_2 | undefined);
    get not(): this | BooleanElement | JSONReferenceElement | undefined | any;
    set not(not: this | BooleanElement | JSONReferenceElement | undefined | any);
    /**
     * Metadata keywords
     *
     * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-01#section-7
     */
    get examples(): ArrayElement | undefined;
    set examples(examples: ArrayElement | undefined);
}

/**
 * @public
 */
export declare class JSONSchemaVisitor extends JSONSchemaVisitor_2 {
    element: JSONSchemaElement;
    constructor(options: JSONSchemaVisitorOptions);
    get defaultDialectIdentifier(): string;
    BooleanElement(booleanElement: BooleanElement): {};
    handleSchemaIdentifier(objectElement: ObjectElement, identifierKeyword?: string): void;
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
    JSONSchemaDraft6Element: string[];
    JSONReferenceElement: string[];
    MediaElement: string[];
    LinkDescriptionElement: string[];
};

/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-01#section-6
 * @public
 */
export declare class LinkDescriptionElement extends LinkDescriptionElement_2 {
    get hrefSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined;
    set hrefSchema(hrefSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined);
    get targetSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined | any;
    set targetSchema(targetSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined | any);
    get schema(): JSONSchemaElement | JSONReferenceElement | undefined;
    set schema(schema: JSONSchemaElement | JSONReferenceElement | undefined);
    get submissionSchema(): JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined;
    set submissionSchema(submissionSchema: JSONSchemaElement | BooleanElement | JSONReferenceElement | undefined);
    get method(): StringElement | undefined;
    set method(method: StringElement | undefined);
    get encType(): StringElement | undefined;
    set encType(encType: StringElement | undefined);
    get submissionEncType(): StringElement | undefined;
    set submissionEncType(submissionEncType: StringElement | undefined);
}

/**
 * @public
 */
export declare class LinkDescriptionVisitor extends LinkDescriptionVisitor_2 {
    readonly element: LinkDescriptionElement;
    constructor(options: LinkDescriptionVisitorOptions);
}

export { LinkDescriptionVisitorOptions }

export { MapVisitor }

export { MapVisitorOptions }

export { MediaElement }

/**
 * @public
 */
export declare const mediaTypes: JSONSchemaDraft6MediaTypes;

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
            value: FallbackVisitor;
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
                            items: JSONSchemaDraft4ItemsVisitor;
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
                            properties: PropertiesVisitor;
                            additionalProperties: SchemaOrReferenceVisitor;
                            patternProperties: PatternPropertiesVisitor;
                            dependencies: DependenciesVisitor;
                            enum: EnumVisitor;
                            type: TypeVisitor;
                            allOf: AllOfVisitor;
                            anyOf: AnyOfVisitor;
                            oneOf: OneOfVisitor;
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

export { SchemaOrReferenceVisitor }

export { SchemaOrReferenceVisitorOptions }

export declare const specificationObj: typeof specificationObj_2;

export { SpecificationVisitor }

export { SpecificationVisitorOptions }

export { SpecPath }

export { Visitor }

export { VisitorOptions }

export { }
