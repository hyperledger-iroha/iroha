import { AnnotationElement } from '@swagger-api/apidom-core';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
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
import { ObjectElement } from '@swagger-api/apidom-core';
import { OpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';
import { OperationElement } from '@swagger-api/apidom-ns-openapi-3-1';
import { ParameterElement } from '@swagger-api/apidom-ns-openapi-3-1';
import { RequestBodyElement } from '@swagger-api/apidom-ns-openapi-3-1';
import { ResponseElement } from '@swagger-api/apidom-ns-openapi-3-1';
import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
declare const apiDesignSystems: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default apiDesignSystems;

/**
 * @public
 */
export declare class ApiDesignSystemsMediaTypes extends MediaTypes<string> {
    filterByFormat(format?: Format): string[];
    findBy(version?: string, format?: Format): string;
    latest(format?: Format): string;
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

/**
 * @public
 */
export declare class InfoElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get title(): StringElement | undefined;
    set title(title: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
}

/**
 * @public
 */
export declare class InfoVisitor extends InfoVisitor_base {
    readonly element: InfoElement;
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

export { isElement }

/**
 * @public
 */
export declare const isInfoElement: ElementPredicate<InfoElement>;

export { isLinkElement }

/**
 * @public
 */
export declare const isMainElement: ElementPredicate<MainElement>;

export { isMemberElement }

export { isNullElement }

export { isNumberElement }

export { isObjectElement }

/**
 * @public
 */
export declare const isPrincipleElement: ElementPredicate<PrincipleElement>;

export { isRefElement }

/**
 * @public
 */
export declare const isRequirementElement: ElementPredicate<RequirementElement>;

/**
 * @public
 */
export declare const isRequirementLevelElement: ElementPredicate<RequirementLevelElement>;

/**
 * @public
 */
export declare const isScenarioElement: ElementPredicate<ScenarioElement>;

/**
 * @public
 */
export declare const isStandardElement: ElementPredicate<StandardElement>;

/**
 * @public
 */
export declare const isStandardIdentifierElement: ElementPredicate<StandardIdentifierElement>;

export { isStringElement }

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
    /**
     * API Design Systems 2021-05-07 specification elements.
     */
    MainElement: string[];
    InfoElement: string[];
    PrincipleElement: string[];
    StandardElement: string[];
    ScenarioElement: string[];
    RequirementElement: string[];
    StandardIdentifierElement: string[];
    RequirementLevelElement: never[];
};

/**
 * @public
 */
export declare class MainElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get version(): StringElement | undefined;
    set version(version: StringElement | undefined);
    get info(): InfoElement | undefined;
    set info(info: InfoElement | undefined);
    get principles(): ArrayElement | undefined;
    set principles(principles: ArrayElement | undefined);
    get standards(): ArrayElement | undefined;
    set standards(standards: ArrayElement | undefined);
    get scenarios(): ArrayElement | undefined;
    set scenarios(scenarios: ArrayElement | undefined);
}

/**
 * @public
 */
export declare class MainPrinciplesVisitor extends PrinciplesVisitor_base {
    readonly element: ArrayElement;
    constructor(options: MainPrinciplesVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface MainPrinciplesVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class MainScenariosVisitor extends ScenariosVisitor_base {
    readonly element: ArrayElement;
    constructor(options: MainScenariosVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface MainScenariosVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class MainStandardsVisitor extends StandardsVisitor_base {
    readonly element: ArrayElement;
    constructor(options: MainStandardsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface MainStandardsVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class MainVisitor extends MainVisitor_base {
    readonly element: MainElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Main']>;
    constructor(options: MainVisitorOptions);
}

declare const MainVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface MainVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare const mediaTypes: ApiDesignSystemsMediaTypes;

/**
 * @public
 */
export declare class PrincipleElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get iri(): StringElement | undefined;
    set iri(iri: StringElement | undefined);
    get level(): RequirementLevelElement | undefined;
    set level(level: RequirementLevelElement | undefined);
}

declare const PrinciplesVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class PrincipleVisitor extends PrincipleVisitor_base {
    readonly element: PrincipleElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Principle']>;
    constructor(options: PrincipleVisitorOptions);
}

declare const PrincipleVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface PrincipleVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare const refractPluginOpenApi3_1StandardIdentifierAccessors: () => () => {
    visitor: {
        OperationElement(element: OperationElement, ...rest: any): void;
        ParameterElement(element: ParameterElement): void;
        RequestBodyElement(element: RequestBodyElement): void;
        ResponseElement(element: ResponseElement): void;
    };
};

/**
 * @public
 */
export declare const refractPluginOpenApi3_1StandardIdentifierSelectors: () => () => {
    visitor: {
        OperationElement: {
            enter(element: OperationElement, ...rest: any): void;
            leave(element: OperationElement): void;
        };
        ParameterElement(element: ParameterElement): void;
        RequestBodyElement(element: RequestBodyElement): void;
        ResponseElement: {
            enter(element: ResponseElement): void;
            leave(element: ResponseElement): void;
        };
    };
};

/**
 * @public
 */
export declare class RequirementElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get subject(): StandardIdentifierElement;
    set subject(subject: StandardIdentifierElement);
    get level(): RequirementLevelElement;
    set level(level: RequirementLevelElement);
    get values(): ArrayElement | undefined;
    set values(values: ArrayElement | undefined);
    get follows(): StringElement | undefined;
    set follows(follows: StringElement | undefined);
}

/**
 * @public
 */
export declare class RequirementLevelElement extends StringElement {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class RequirementLevelVisitor extends RequirementLevelVisitor_base {
    element: RequirementLevelElement;
    StringElement(stringElement: StringElement): {};
}

declare const RequirementLevelVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface RequirementLevelVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class RequirementVisitor extends RequirementVisitor_base {
    readonly element: RequirementElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Requirement']>;
    constructor(options: RequirementVisitorOptions);
}

declare const RequirementVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface RequirementVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ScenarioElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get when(): StandardIdentifierElement;
    set when(when: StandardIdentifierElement);
    get then(): ArrayElement | undefined;
    set then(then: ArrayElement | undefined);
}

declare const ScenariosVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class ScenarioThenVisitor extends ThenVisitor_base {
    readonly element: ArrayElement;
    constructor(options: ScenarioThenVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface ScenarioThenVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ScenarioVisitor extends ScenarioVisitor_base {
    readonly element: ScenarioElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Scenario']>;
    constructor(options: ScenarioVisitorOptions);
}

declare const ScenarioVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ScenarioVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
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
        document: {
            objects: {
                /**
                 * API Design Systems 2021-05-07 specification elements.
                 */
                Main: {
                    $visitor: typeof MainVisitor;
                    fixedFields: {
                        version: {
                            $ref: string;
                        };
                        info: {
                            $ref: string;
                        };
                        principles: typeof MainPrinciplesVisitor;
                        standards: typeof MainStandardsVisitor;
                        scenarios: typeof MainScenariosVisitor;
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
                    };
                };
                Principle: {
                    $visitor: typeof PrincipleVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        iri: {
                            $ref: string;
                        };
                        level: {
                            $ref: string;
                        };
                    };
                };
                Standard: {
                    $visitor: typeof StandardVisitor;
                    fixedFields: {
                        name: {
                            $ref: string;
                        };
                        description: {
                            $ref: string;
                        };
                        level: {
                            $ref: string;
                        };
                        iri: {
                            $ref: string;
                        };
                    };
                };
                Scenario: {
                    $visitor: typeof ScenarioVisitor;
                    fixedFields: {
                        description: {
                            $ref: string;
                        };
                        when: {
                            $ref: string;
                        };
                        then: typeof ScenarioThenVisitor;
                    };
                };
                Requirement: {
                    $visitor: typeof RequirementVisitor;
                    fixedFields: {
                        subject: {
                            $ref: string;
                        };
                        level: {
                            $ref: string;
                        };
                        values: {
                            $ref: string;
                        };
                        follows: {
                            $ref: string;
                        };
                    };
                };
                StandardIdentifier: {
                    $visitor: typeof StandardIdentifierVisitor;
                };
                RequirementLevel: {
                    $visitor: typeof RequirementLevelVisitor;
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
    retrievePassingOptions(): Pick<this, keyof this>;
    retrieveFixedFields(specPath: string[]): string[];
    retrieveVisitor(specPath: string[]): unknown;
    retrieveVisitorInstance(specPath: string[], options?: {}): Visitor;
    toRefractedElement(specPath: string[], element: any, options?: {}): any;
}

/**
 * This is a base class for every visitor that does
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
export declare class StandardElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get iri(): StringElement | undefined;
    set iri(iri: StringElement | undefined);
    get level(): RequirementLevelElement | undefined;
    set level(level: RequirementLevelElement | undefined);
}

/**
 * @public
 */
export declare class StandardIdentifierElement extends ArrayElement {
    constructor(content?: string[], meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StandardIdentifierVisitor extends StandardIdentifierVisitor_base {
    readonly element: StandardIdentifierElement;
    constructor(options: StandardIdentifierVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const StandardIdentifierVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface StandardIdentifierVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

declare const StandardsVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class StandardVisitor extends StandardVisitor_base {
    readonly element: StandardElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Standard']>;
    constructor(options: StandardVisitorOptions);
}

declare const StandardVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface StandardVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

declare const ThenVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare const validateOpenAPI3_1: (mainElement: MainElement, openApiElement: OpenApi3_1Element) => AnnotationElement[];

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
export { VisitorOptions as FallbackVisitorOptions }
export { VisitorOptions }

export { }
