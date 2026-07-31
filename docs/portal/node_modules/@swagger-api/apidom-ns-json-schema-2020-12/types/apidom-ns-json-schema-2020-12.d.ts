import { $defsVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { $defsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { $refVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { $refVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { $vocabularyVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { $vocabularyVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { AllOfVisitor as AllOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AllOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { AlternatingVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { AlternatingVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { AnyOfVisitor as AnyOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { BooleanElement } from '@swagger-api/apidom-core';
import { Class } from 'ts-mixer/dist/types/types.js';
import { DefinitionsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependentRequiredVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { DependentRequiredVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { DependentSchemasVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { DependentSchemasVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { FallbackVisitor as FallbackVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { FallbackVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { FixedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { FixedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
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
import { ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { ItemsVisitor as ItemsVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ItemsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { JSONReference$RefVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaElement as JSONSchemaElement_2 } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { JSONSchemaVisitor as JSONSchemaVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaVisitor as JSONSchemaVisitor_3 } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { JSONSchemaVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { LinkDescriptionElement as LinkDescriptionElement_2 } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { LinkDescriptionVisitor as LinkDescriptionVisitor_3 } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { LinkDescriptionVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { LinksVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MapVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { MapVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MediaVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { OneOfVisitor as OneOfVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { OneOfVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { ParentSchemaAwareVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { PatternedFieldsVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { PatternedFieldsVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { PatternPropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { PatternPropertiesVisitor as PatternPropertiesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternPropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { PropertiesVisitor as PropertiesVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { specificationObj as specificationObj_2 } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { SpecificationVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { SpecificationVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { SpecPath } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { StringElement } from '@swagger-api/apidom-core';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Visitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import { VisitorOptions } from '@swagger-api/apidom-ns-json-schema-2019-09';

export { $defsVisitor }

export { $defsVisitorOptions }

export { $refVisitor }

export { $refVisitorOptions }

export { $vocabularyVisitor }

export { $vocabularyVisitorOptions }

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

export { DependentRequiredVisitor }

export { DependentRequiredVisitorOptions }

export { DependentSchemasVisitor }

export { DependentSchemasVisitorOptions }

export { FallbackVisitor }

export { FallbackVisitorOptions }

export { FixedFieldsVisitor }

export { FixedFieldsVisitorOptions }

/**
 * @public
 */
export declare type Format = 'generic' | 'json' | 'yaml';

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

export { ItemsVisitor }

export { ItemsVisitorOptions }

/**
 * @public
 */
declare const jsonSchema202012: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default jsonSchema202012;

/**
 * @public
 */
export declare class JSONSchema202012MediaTypes extends MediaTypes<string> {
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
     * URI: https://json-schema.org/draft/2020-12/vocab/core
     */
    get $dynamicAnchor(): StringElement | undefined;
    set $dynamicAnchor($dynamicAnchor: StringElement | undefined);
    get $recursiveAnchor(): BooleanElement | undefined;
    set $recursiveAnchor($recursiveAnchor: BooleanElement | undefined);
    get $dynamicRef(): StringElement | undefined;
    set $dynamicRef($dynamicRef: StringElement | undefined);
    get $recursiveRef(): StringElement | undefined;
    set $recursiveRef($recursiveRef: StringElement | undefined);
    /**
     * Applicator vocabulary
     *
     * URI: https://json-schema.org/draft/2020-12/vocab/applicator
     */
    get prefixItems(): ArrayElement | undefined;
    set prefixItems(prefixItems: ArrayElement | undefined);
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
export declare const mediaTypes: JSONSchema202012MediaTypes;

export { OneOfVisitor }

export { OneOfVisitorOptions }

export { ParentSchemaAwareVisitor }

export { ParentSchemaAwareVisitorOptions }

export { PatternedFieldsVisitor }

export { PatternedFieldsVisitorOptions }

export { PatternPropertiesVisitor }

export { PatternPropertiesVisitorOptions }

/**
 * @public
 */
export declare class PrefixItemsVisitor extends PrefixItemsVisitor_base {
    readonly element: ArrayElement;
    constructor(options: PrefixItemsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const PrefixItemsVisitor_base: Class<any[], SpecificationVisitor & ParentSchemaAwareVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof ParentSchemaAwareVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PrefixItemsVisitorOptions extends SpecificationVisitorOptions, ParentSchemaAwareVisitorOptions, FallbackVisitorOptions {
}

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
