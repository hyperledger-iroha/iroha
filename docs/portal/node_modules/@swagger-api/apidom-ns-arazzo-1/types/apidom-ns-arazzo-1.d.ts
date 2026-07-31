import { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { ArrayElement } from '@swagger-api/apidom-core';
import { Attributes } from '@swagger-api/apidom-core';
import { Class } from 'ts-mixer/dist/types/types.js';
import { DefinitionsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { DependenciesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { Element as Element_2 } from '@swagger-api/apidom-core';
import { ElementPredicate } from '@swagger-api/apidom-core';
import { EnumVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isArrayElement } from '@swagger-api/apidom-core';
import { isBooleanElement } from '@swagger-api/apidom-core';
import { isElement } from '@swagger-api/apidom-core';
import { isJSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { isLinkElement as isLinkPrimitiveElement } from '@swagger-api/apidom-core';
import { isMemberElement } from '@swagger-api/apidom-core';
import { isNullElement } from '@swagger-api/apidom-core';
import { isNumberElement } from '@swagger-api/apidom-core';
import { isObjectElement } from '@swagger-api/apidom-core';
import { isRefElement } from '@swagger-api/apidom-core';
import { isStringElement } from '@swagger-api/apidom-core';
import { ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { default as JSONSchemaVisitor } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { JSONSchemaVisitor as JSONSchemaVisitor_2 } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { JSONSchemaVisitorOptions } from '@swagger-api/apidom-ns-json-schema-2020-12';
import { LinksVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { MediaTypes } from '@swagger-api/apidom-core';
import { MemberElement } from '@swagger-api/apidom-core';
import { Meta } from '@swagger-api/apidom-core';
import { Namespace } from 'minim';
import { NamespacePluginOptions } from '@swagger-api/apidom-core';
import { NumberElement } from '@swagger-api/apidom-core';
import { ObjectElement } from '@swagger-api/apidom-core';
import { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PatternPropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { RequiredVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { SchemaOrReferenceVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { StringElement } from '@swagger-api/apidom-core';
import { TypeVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';

/**
 * @public
 */
declare const arazzo1: {
    namespace: (options: NamespacePluginOptions) => Namespace;
};
export default arazzo1;

/**
 * @public
 */
export declare class ArazzoMediaTypes extends MediaTypes<string> {
    filterByFormat(format?: Format): string[];
    findBy(version?: string, format?: Format): string;
    latest(format?: Format): string;
}

/**
 * @public
 */
export declare class ArazzoParametersVisitor extends ParametersVisitor_base_3 {
    readonly element: ArrayElement;
    constructor(options: ArazzoParametersVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface ArazzoParametersVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ArazzoSourceDescriptionsVisitor extends SourceDescriptionsVisitor_base {
    readonly element: SourceDescriptionsElement;
    constructor(options: ArazzoSourceDescriptionsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface ArazzoSourceDescriptionsVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ArazzoSpecElement extends StringElement {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ArazzoSpecification1Element extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get arazzoSpec(): ArazzoSpecElement | undefined;
    set arazzoSpec(arazzoSpec: ArazzoSpecElement | undefined);
    get info(): InfoElement | undefined;
    set info(info: InfoElement | undefined);
    get sourceDescriptions(): ArrayElement | undefined;
    set sourceDescriptions(sourceDescriptions: ArrayElement | undefined);
    get workflows(): ArrayElement | undefined;
    set workflows(workflows: ArrayElement | undefined);
    get components(): ComponentsElement | undefined;
    set components(components: ComponentsElement | undefined);
}

/**
 * @public
 */
export declare class ArazzoSpecificationVisitor extends ArazzoSpecificationVisitor_base {
    readonly element: ArazzoSpecification1Element;
    protected readonly specPath: SpecPath<['document', 'objects', 'ArazzoSpecification']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: ArazzoSpecificationVisitorOptions);
    ObjectElement(objectElement: ObjectElement): {};
}

declare const ArazzoSpecificationVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ArazzoSpecificationVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ArazzoSpecVisitor extends ArazzoSpecVisitor_base {
    element: ArazzoSpecElement;
    StringElement(stringElement: StringElement): {};
}

declare const ArazzoSpecVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface ArazzoSpecVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ArazzoStepsVisitor extends StepsVisitor_base {
    readonly element: WorkflowStepsElement;
    constructor(options: ArazzoStepsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface ArazzoStepsVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ArazzoSuccessActionCriteriaVisitor extends SuccessActionCriteriaVisitor_base {
    readonly element: SuccessActionCriteriaElement;
    constructor(options: ArazzoSuccessActionCriteriaVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface ArazzoSuccessActionCriteriaVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get inputs(): ObjectElement | undefined;
    set inputs(inputs: ObjectElement | undefined);
    get parameters(): ObjectElement | undefined;
    set parameters(parameters: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class ComponentsInputsVisitor extends InputsVisitor_base {
    readonly element: ComponentsSchemas;
    protected readonly specPath: SpecPath<['document', 'objects', 'JSONSchema']>;
    constructor(options: ComponentsInputsVisitorOptions);
}

/**
 * @public
 */
export declare interface ComponentsInputsVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsParameters extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class ComponentsParametersVisitor extends ParametersVisitor_base_2 {
    readonly element: ComponentsParameters;
    protected readonly specPath: SpecPath<['document', 'objects', 'Parameter']>;
    constructor(options: ComponentsParametersVisitorOptions);
}

/**
 * @public
 */
export declare interface ComponentsParametersVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class ComponentsSchemas extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
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
export declare const createRefractor: (specPath: string[]) => (value: unknown, options?: {}) => Element_2;

/**
 * @public
 */
export declare class CriterionElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get context(): StringElement | undefined;
    set context(context: StringElement | undefined);
    get condition(): StringElement | undefined;
    set condition(condition: StringElement | undefined);
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
}

/**
 * @public
 */
export declare class CriterionVisitor extends CriterionVisitor_base {
    readonly element: CriterionElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Criterion']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: CriterionVisitorOptions);
}

declare const CriterionVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface CriterionVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class FailureActionCriteriaElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class FailureActionCriteriaVisitor extends FailureActionCriteriaVisitor_base {
    readonly element: FailureActionCriteriaElement;
    constructor(options: FailureActionCriteriaVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const FailureActionCriteriaVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface FailureActionCriteriaVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class FailureActionElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get workflowId(): StringElement | undefined;
    set workflowId(workflowId: StringElement | undefined);
    get stepId(): StringElement | undefined;
    set stepId(stepId: StringElement | undefined);
    get retryAfter(): NumberElement | undefined;
    set retryAfter(retryAfter: NumberElement | undefined);
    get retryLimit(): NumberElement | undefined;
    set retryLimit(retryLimit: NumberElement | undefined);
    get criteria(): ArrayElement | undefined;
    set criteria(criteria: ArrayElement | undefined);
}

declare const FailureActionVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface FailureActionVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class FailureVisitor extends FailureActionVisitor_base {
    readonly element: FailureActionElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'FailureAction']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: FailureActionVisitorOptions);
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
    readonly specificationExtensionPredicate?: typeof isArazzoSpecificationExtension;
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
    get summary(): StringElement | undefined;
    set summary(summary: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
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

declare const InputsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare const isArazzoSpecElement: ElementPredicate<ArazzoSpecElement>;

/**
 * @public
 */
export declare const isArazzoSpecification1Element: ElementPredicate<ArazzoSpecification1Element>;

/**
 * @public
 */
export declare const isArazzoSpecificationExtension: (element: MemberElement) => boolean;

export { isArrayElement }

export { isBooleanElement }

/**
 * @public
 */
export declare const isComponentsElement: ElementPredicate<ComponentsElement>;

/**
 * @public
 */
export declare const isCriterionElement: ElementPredicate<CriterionElement>;

export { isElement }

/**
 * @public
 */
export declare const isFailureActionCriteriaElement: ElementPredicate<FailureActionCriteriaElement>;

/**
 * @public
 */
export declare const isFailureActionElement: ElementPredicate<FailureActionElement>;

/**
 * @public
 */
export declare const isInfoElement: ElementPredicate<InfoElement>;

export { isJSONSchemaElement }

export { isLinkPrimitiveElement }

export { isMemberElement }

export { isNullElement }

export { isNumberElement }

export { isObjectElement }

/**
 * @public
 */
export declare const isParameterElement: ElementPredicate<ParameterElement>;

export { isRefElement }

/**
 * @public
 */
export declare const isReferenceElement: ElementPredicate<ReferenceElement>;

/**
 * @public
 */
export declare const isSourceDescriptionElement: ElementPredicate<SourceDescriptionElement>;

/**
 * @public
 */
export declare const isSourceDescriptionsElement: ElementPredicate<SourceDescriptionsElement>;

/**
 * @public
 */
export declare const isStepDependsOnElement: ElementPredicate<StepDependsOnElement>;

/**
 * @public
 */
export declare const isStepElement: ElementPredicate<StepElement>;

/**
 * @public
 */
export declare const isStepOnFailureElement: ElementPredicate<StepOnFailureElement>;

/**
 * @public
 */
export declare const isStepOnSuccessElement: ElementPredicate<StepOnSuccessElement>;

/**
 * @public
 */
export declare const isStepOutputsElement: ElementPredicate<StepOutputsElement>;

/**
 * @public
 */
export declare const isStepParametersElement: ElementPredicate<StepParametersElement>;

/**
 * @public
 */
export declare const isStepSuccessCriteriaElement: ElementPredicate<StepSuccessCriteriaElement>;

export { isStringElement }

/**
 * @public
 */
export declare const isSuccessActionCriteriaElement: ElementPredicate<SuccessActionCriteriaElement>;

/**
 * @public
 */
export declare const isSuccessActionElement: ElementPredicate<SuccessActionElement>;

/**
 * @public
 */
export declare const isWorkflowElement: ElementPredicate<WorkflowElement>;

/**
 * @public
 */
export declare const isWorkflowOutputsElement: ElementPredicate<WorkflowOutputsElement>;

/**
 * @public
 */
export declare const isWorkflowStepsElement: ElementPredicate<WorkflowStepsElement>;

export { JSONSchemaElement }

export { JSONSchemaVisitor }

export { JSONSchemaVisitorOptions }

/**
 * Arazzo Specification 1.0.1
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
    ArazzoSpecification1Element: string[];
    InfoElement: string[];
    SourceDescriptionElement: string[];
    WorkflowElement: string[];
    StepElement: string[];
    ParameterElement: string[];
    SuccessActionElement: string[];
    FailureActionElement: string[];
    ComponentsElement: string[];
    CriterionElement: string[];
    ReferenceElement: string[];
    JSONSchema202012Element: string[];
};

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
export declare const mediaTypes: ArazzoMediaTypes;

declare const OnFailureVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const OnSuccessVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const OutputsVisitor_base: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const OutputsVisitor_base_2: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class ParameterElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get in(): StringElement | undefined;
    set in(val: StringElement | undefined);
    get style(): StringElement | undefined;
    set style(style: StringElement | undefined);
    get target(): StringElement | undefined;
    set target(target: StringElement | undefined);
    get value(): Element_2 | undefined;
    set value(value: Element_2 | undefined);
}

declare const ParametersVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const ParametersVisitor_base_2: Class<any[], MapVisitor & FallbackVisitor, typeof MapVisitor & typeof FallbackVisitor>;

declare const ParametersVisitor_base_3: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

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
    readonly specificationExtensionPredicate?: typeof isArazzoSpecificationExtension;
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
    get value(): StringElement | undefined;
    set value(value: StringElement | undefined);
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
export declare const refract: <T extends Element_2>(value: unknown, { specPath, plugins, }?: {
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
export declare class SourceDescriptionElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get name(): StringElement | undefined;
    set name(name: StringElement | undefined);
    get url(): StringElement | undefined;
    set url(url: StringElement | undefined);
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
}

/**
 * @public
 */
export declare class SourceDescriptionsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class SourceDescriptionsUrlVisitor extends FallbackVisitor {
    readonly element: StringElement;
    StringElement(stringElement: StringElement): {};
}

/**
 * @public
 */
export declare class SourceDescriptionsVisitor extends SourceDescriptionVisitor_base {
    readonly element: SourceDescriptionElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'SourceDescription']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: SourceDescriptionVisitorOptions);
}

declare const SourceDescriptionsVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const SourceDescriptionVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SourceDescriptionVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
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
                readonly ArazzoSpecification: {
                    readonly $visitor: typeof ArazzoSpecificationVisitor;
                    readonly fixedFields: {
                        readonly arazzo: typeof ArazzoSpecVisitor;
                        readonly info: {
                            readonly $ref: "#/visitors/document/objects/Info";
                        };
                        readonly sourceDescriptions: typeof ArazzoSourceDescriptionsVisitor;
                        readonly workflows: typeof WorkflowsVisitor;
                        readonly components: {
                            readonly $ref: "#/visitors/document/objects/Components";
                        };
                    };
                };
                readonly Info: {
                    readonly $visitor: typeof InfoVisitor;
                    readonly fixedFields: {
                        readonly title: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly version: typeof InfoVersionVisitor;
                    };
                };
                readonly SourceDescription: {
                    readonly $visitor: typeof SourceDescriptionsVisitor;
                    readonly fixedFields: {
                        readonly name: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly url: typeof SourceDescriptionsUrlVisitor;
                        readonly type: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Workflow: {
                    readonly $visitor: typeof WorkflowVisitor;
                    readonly fixedFields: {
                        readonly workflowId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly summary: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly inputs: {
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
                        readonly steps: typeof ArazzoStepsVisitor;
                        readonly outputs: typeof WorkflowOutputsVisitor;
                    };
                };
                readonly Step: {
                    readonly $visitor: typeof StepsVisitor;
                    readonly fixedFields: {
                        readonly description: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly stepId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly operationId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly operationRef: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly workflowId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly parameters: typeof StepParametersVisitor;
                        readonly dependsOn: typeof StepDependsOnVisitor;
                        readonly successCriteria: typeof StepSuccessCriteriaVisitor;
                        readonly onSuccess: typeof StepOnSuccessVisitor;
                        readonly onFailure: typeof StepOnFailureVisitor;
                        readonly outputs: typeof StepOutputsVisitor;
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
                        readonly style: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly target: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly value: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly SuccessAction: {
                    readonly $visitor: typeof SuccessActionVisitor;
                    readonly fixedFields: {
                        readonly type: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly workflowId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly stepId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly criteria: typeof ArazzoSuccessActionCriteriaVisitor;
                    };
                };
                readonly FailureAction: {
                    readonly $visitor: typeof FailureVisitor;
                    readonly fixedFields: {
                        readonly type: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly workflowId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly stepId: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly retryAfter: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly retryLimit: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly criteria: typeof FailureActionCriteriaVisitor;
                    };
                };
                readonly Components: {
                    readonly $visitor: typeof ComponentsVisitor;
                    readonly fixedFields: {
                        readonly inputs: typeof ComponentsInputsVisitor;
                        readonly parameters: typeof ComponentsParametersVisitor;
                    };
                };
                readonly Criterion: {
                    readonly $visitor: typeof CriterionVisitor;
                    readonly fixedFields: {
                        readonly context: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly condition: {
                            readonly $ref: "#/visitors/value";
                        };
                        readonly type: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly Reference: {
                    readonly $visitor: typeof ReferenceVisitor;
                    readonly fixedFields: {
                        readonly $ref: typeof Reference$RefVisitor;
                        readonly value: {
                            readonly $ref: "#/visitors/value";
                        };
                    };
                };
                readonly JSONSchema: {
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
declare interface SpecificationVisitorOptions extends VisitorOptions {
    readonly specObj: typeof specificationObj;
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
export declare class StepDependsOnElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StepDependsOnVisitor extends FallbackVisitor {
    element: StepDependsOnElement;
    constructor(options: StepDependsOnVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface StepDependsOnVisitorOptions extends VisitorOptions {
}

/**
 * @public
 */
export declare class StepElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get stepId(): StringElement | undefined;
    set stepId(stepId: StringElement | undefined);
    get operationId(): StringElement | undefined;
    set operationId(operationId: StringElement | undefined);
    get operationRef(): StringElement | undefined;
    set operationRef(operationRef: StringElement | undefined);
    get workflowId(): StringElement | undefined;
    set workflowId(workflowId: StringElement | undefined);
    get parameters(): ArrayElement | undefined;
    set parameters(parameters: ArrayElement | undefined);
    get dependsOn(): ArrayElement | undefined;
    set dependsOn(dependsOn: ArrayElement | undefined);
    get successCriteria(): ArrayElement | undefined;
    set successCriteria(successCriteria: ArrayElement | undefined);
    get onSuccess(): ArrayElement | undefined;
    set onSuccess(onSuccess: ArrayElement | undefined);
    get onFailure(): ArrayElement | undefined;
    set onFailure(onFailure: ArrayElement | undefined);
    get outputs(): ObjectElement | undefined;
    set outputs(outputs: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class StepOnFailureElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StepOnFailureVisitor extends OnFailureVisitor_base {
    readonly element: StepOnFailureElement;
    constructor(options: StepOnFailureVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface StepOnFailureVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class StepOnSuccessElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StepOnSuccessVisitor extends OnSuccessVisitor_base {
    readonly element: StepOnSuccessElement;
    constructor(options: StepOnSuccessVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface StepOnSuccessVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class StepOutputsElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StepOutputsVisitor extends OutputsVisitor_base_2 {
    readonly element: StepOutputsElement;
    protected readonly specPath: SpecPath<['value']>;
    constructor(options: StepOutputsVisitorOptions);
}

/**
 * @public
 */
export declare interface StepOutputsVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class StepParametersElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StepParametersVisitor extends ParametersVisitor_base {
    readonly element: StepParametersElement;
    constructor(options: StepParametersVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface StepParametersVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class StepSuccessCriteriaElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class StepSuccessCriteriaVisitor extends SuccessCriteriaVisitor_base {
    readonly element: StepSuccessCriteriaElement;
    constructor(options: StepSuccessCriteriaVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

/**
 * @public
 */
export declare interface StepSuccessCriteriaVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class StepsVisitor extends StepVisitor_base {
    readonly element: StepElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Step']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: StepVisitorOptions);
}

declare const StepsVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

declare const StepVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface StepVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class SuccessActionCriteriaElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

declare const SuccessActionCriteriaVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare class SuccessActionElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get type(): StringElement | undefined;
    set type(type: StringElement | undefined);
    get workflowId(): StringElement | undefined;
    set workflowId(workflowId: StringElement | undefined);
    get stepId(): StringElement | undefined;
    set stepId(stepId: StringElement | undefined);
    get criteria(): ArrayElement | undefined;
    set criteria(criteria: ArrayElement | undefined);
}

/**
 * @public
 */
export declare class SuccessActionVisitor extends SuccessActionVisitor_base {
    readonly element: SuccessActionElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'SuccessAction']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: SuccessActionVisitorOptions);
}

declare const SuccessActionVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface SuccessActionVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

declare const SuccessCriteriaVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

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
export { VisitorOptions as InfoVersionVisitorOptions }
export { VisitorOptions as Reference$RefVisitorOptions }
export { VisitorOptions as SourceDescriptionsUrlVisitorOptions }
export { VisitorOptions }

/**
 * @public
 */
export declare class WorkflowElement extends ObjectElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
    get workflowId(): StringElement | undefined;
    set workflowId(workflowId: StringElement | undefined);
    get summary(): StringElement | undefined;
    set summary(summary: StringElement | undefined);
    get description(): StringElement | undefined;
    set description(description: StringElement | undefined);
    get inputs(): JSONSchemaElement | undefined;
    set inputs(inputs: JSONSchemaElement | undefined);
    get steps(): ArrayElement | undefined;
    set steps(steps: ArrayElement | undefined);
    get outputs(): ObjectElement | undefined;
    set outputs(outputs: ObjectElement | undefined);
}

/**
 * @public
 */
export declare class WorkflowOutputsElement extends ObjectElement {
    static primaryClass: string;
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class WorkflowOutputsVisitor extends OutputsVisitor_base {
    readonly element: WorkflowOutputsElement;
    protected readonly specPath: SpecPath<['value']>;
    constructor(options: WorkflowOutputsVisitorOptions);
}

/**
 * @public
 */
export declare interface WorkflowOutputsVisitorOptions extends MapVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class WorkflowsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class WorkflowStepsElement extends ArrayElement {
    static primaryClass: string;
    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare class WorkflowsVisitor extends WorkflowsVisitor_base {
    readonly element: WorkflowsElement;
    constructor(options: WorkflowsVisitorOptions);
    ArrayElement(arrayElement: ArrayElement): {};
}

declare const WorkflowsVisitor_base: Class<any[], SpecificationVisitor & FallbackVisitor, typeof SpecificationVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface WorkflowsVisitorOptions extends SpecificationVisitorOptions, VisitorOptions {
}

/**
 * @public
 */
export declare class WorkflowVisitor extends WorkflowVisitor_base {
    readonly element: WorkflowElement;
    protected readonly specPath: SpecPath<['document', 'objects', 'Workflow']>;
    protected readonly canSupportSpecificationExtensions: true;
    constructor(options: WorkflowVisitorOptions);
}

declare const WorkflowVisitor_base: Class<any[], FixedFieldsVisitor & FallbackVisitor, typeof FixedFieldsVisitor & typeof FallbackVisitor>;

/**
 * @public
 */
export declare interface WorkflowVisitorOptions extends FixedFieldsVisitorOptions, VisitorOptions {
}

export { }
