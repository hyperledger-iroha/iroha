/// <reference path="./minim.d.ts" />
import * as minim from "minim";
import type { ApiDOMErrorOptions } from '@swagger-api/apidom-error';
import { ApiDOMStructuredError } from '@swagger-api/apidom-error';
import { ArrayElement } from 'minim';
import { ArraySlice } from 'minim';
import { Attributes } from 'minim';
import { BooleanElement } from 'minim';
import { BREAK } from '@swagger-api/apidom-ast';
import { Element as Element_2 } from 'minim';
import { KeyValuePair } from 'minim';
import { LinkElement } from 'minim';
import { MemberElement } from 'minim';
import { mergeAllVisitors } from '@swagger-api/apidom-ast';
import { Meta } from 'minim';
import { Namespace as Namespace_2 } from 'minim';
import { NamespacePlugin } from 'minim';
import { NamespacePluginOptions } from 'minim';
import { NullElement } from 'minim';
import { NumberElement } from 'minim';
import { ObjectElement } from 'minim';
import { ObjectSlice } from 'minim';
import { RefElement } from 'minim';
import { refract } from 'minim';
import ShortUniqueId from 'short-unique-id';
import { StringElement } from 'minim';

/**
 * @public
 */
export declare class AnnotationElement extends StringElement {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
    get code(): any;
    set code(value: any);
}

/**
 * @public
 */
export declare type AnyElement = ObjectElement | ArrayElement | Element_2;

export { ArrayElement }

export { ArraySlice }

/**
 * @public
 */
export declare const assignSourceMap: (to: SourceMapContainer, from?: SourceMapContainer) => SourceMapContainer;

export { Attributes }

declare namespace basePredicates {
    export {
        isElement,
        isStringElement,
        isNumberElement,
        isNullElement,
        isBooleanElement,
        isObjectElement,
        isArrayElement,
        isMemberElement,
        isLinkElement,
        isRefElement,
        isAnnotationElement,
        isCommentElement,
        isParseResultElement,
        PrimitiveElement,
        isPrimitiveElement,
        hasElementSourceMap,
        includesSymbols,
        includesClasses
    }
}
export { basePredicates }

export { BooleanElement }

export { BREAK }

/**
 * @public
 */
export declare type Callback = <T extends Element_2>(element: T) => void;

/**
 * @public
 */
export declare const cloneDeep: {
    <T extends Element_2 | FinalCloneTypes>(value: T, options?: DeepCloneOptions<T>): T;
    safe<T>(value: T): T;
};

/**
 * @public
 */
export declare class CloneError extends ApiDOMStructuredError {
    readonly value: unknown;
    constructor(message?: string, structuredOptions?: CloneErrorOptions);
}

/**
 * @public
 */
export declare interface CloneErrorOptions extends ApiDOMErrorOptions {
    readonly value: unknown;
}

/**
 * @public
 */
export declare const cloneNode: <T>(node: T) => T;

/**
 * @public
 */
export declare const cloneShallow: {
    <T extends Element_2 | FinalCloneTypes>(value: T): T;
    safe<T>(value: T): T;
};

/**
 * @public
 */
export declare class CommentElement extends StringElement {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
}

/**
 * @public
 */
export declare const createNamespace: (namespacePlugin?: NamespacePlugin) => Namespace;

/**
 * @public
 */
export declare const createPredicate: <T extends Element_2>(predicateCreator: PredicateCreator<T>) => ElementPredicate<T>;

/**
 * @public
 */
export declare const createToolbox: () => Toolbox;

/**
 * @public
 */
export declare class DeepCloneError extends CloneError {
}

/**
 * @public
 */
export declare type DeepCloneOptions<T extends Element_2 | FinalCloneTypes> = {
    visited?: WeakMap<T, T>;
};

/**
 * @public
 */
export declare type DeepMerge = (targetElement: ObjectOrArrayElement, sourceElement: ObjectOrArrayElement, options?: DeepMergeOptions) => AnyElement;

/**
 * @public
 */
export declare const deepmerge: {
    (targetElement: ObjectOrArrayElement, sourceElement: ObjectOrArrayElement, options?: DeepMergeUserOptions): AnyElement;
    all(list: ObjectOrArrayElement[], options?: DeepMergeUserOptions): any;
};

/**
 * @public
 */
export declare type DeepMergeArrayElementMerge = (targetElement: ArrayElement, sourceElement: ArrayElement, options: DeepMergeOptions) => ArrayElement;

/**
 * @public
 */
export declare type DeepMergeCustomAttributesMerge = (targetElementAttributes: ObjectElement, sourceElementAttributes: ObjectElement) => ObjectElement;

/**
 * @public
 */
export declare type DeepMergeCustomMerge = (keyElement: Element_2, options: DeepMergeOptions) => DeepMerge;

/**
 * @public
 */
export declare type DeepMergeCustomMetaMerge = (targetElementMeta: ObjectElement, sourceElementMeta: ObjectElement) => ObjectElement;

/**
 * @public
 */
export declare type DeepMergeObjectElementMerge = (targetElement: ObjectElement, source: ObjectElement, options: DeepMergeOptions) => ObjectElement;

/**
 * @public
 */
export declare type DeepMergeOptions = DeepMergeUserOptions & {
    clone: boolean;
    isMergeableElement: (element: Element_2) => boolean;
    arrayElementMerge: DeepMergeArrayElementMerge;
    objectElementMerge: DeepMergeObjectElementMerge;
    customMerge: DeepMergeCustomMerge | undefined;
    customMetaMerge: DeepMergeCustomMetaMerge | undefined;
    customAttributesMerge: DeepMergeCustomAttributesMerge | undefined;
};

/**
 * @public
 */
export declare type DeepMergeUserOptions = {
    clone?: boolean;
    isMergeableElement?: (element: Element_2) => boolean;
    arrayElementMerge?: DeepMergeArrayElementMerge;
    objectElementMerge?: DeepMergeObjectElementMerge;
    customMerge?: DeepMergeCustomMerge;
    customMetaMerge?: DeepMergeCustomMetaMerge;
    customAttributesMerge?: DeepMergeCustomAttributesMerge;
};

/**
 * @public
 */
export declare const defaultIdentityManager: IdentityManager;

/**
 * Creates a refract representation of an Element.
 * https://github.com/refractproject/refract-spec
 * @public
 */
export declare const dehydrate: (element: Element_2, namespace?: Namespace_2) => any;

/**
 * This dereference algorithm is used exclusively for dereferencing specification objects.
 * It doesn't handle circular references of external references and works on objects only (not arrays).
 * @public
 */
export declare const dereference: (object: Record<string, any>, root?: Record<string, any>) => Record<string, any>;

/**
 * @public
 */
export declare interface DispatchPluginsAsync {
    <T extends Element_2, U extends Element_2 = Element_2>(element: T, plugins: ((toolbox: any) => object)[], options?: Record<string, unknown>): Promise<U>;
}

/**
 * @public
 */
export declare interface DispatchPluginsOptions {
    toolboxCreator: typeof createToolbox;
    visitorOptions: {
        nodeTypeGetter: typeof getNodeType;
        exposeEdits: boolean;
    };
}

/**
 * @public
 */
export declare interface DispatchPluginsSync {
    <T extends Element_2, U extends Element_2 = Element_2>(element: T, plugins: ((toolbox: any) => object)[], options?: Record<string, unknown>): U;
    [key: symbol]: DispatchPluginsAsync;
}

/**
 * @public
 */
export declare const dispatchRefractorPlugins: DispatchPluginsSync;

export { Element_2 as Element }

/**
 * @public
 */
export declare interface ElementBasicsTrait {
    _storedElement: string;
    _content: unknown;
}

/**
 * @public
 */
export declare interface ElementClassesTrait {
    classes: ArrayElement | Array<string>;
}

/**
 * @public
 */
export declare class ElementIdentityError extends ApiDOMStructuredError {
    readonly value: unknown;
    constructor(message?: string, structuredOptions?: ElementIdentityErrorOptions);
}

/**
 * @public
 */
export declare interface ElementIdentityErrorOptions extends ApiDOMErrorOptions {
    readonly value: unknown;
}

/**
 * @public
 */
export declare type ElementPredicate<T extends Element_2> = (element: unknown) => element is T;

/**
 * @public
 */
export declare interface ElementPrimitiveBehavior {
    primitive: () => unknown;
}

/**
 * @public
 */
export declare interface ElementTypeTrait<T = string> {
    element: T;
}

/**
 * Finds all elements matching the predicate.
 * @public
 */
export declare const filter: <T extends Element_2>(predicate: (element: any) => boolean, element: T) => ArraySlice;

/**
 * @public
 */
export declare type FinalCloneTypes = KeyValuePair | ArraySlice | ObjectSlice;

/**
 * Find first element that satisfies the provided predicate.
 * @public
 */
export declare const find: <T extends Element_2>(predicate: (element: any) => boolean, element: T) => T | undefined;

/**
 * Finds the most inner node at the given offset.
 * If includeRightBound is set, also finds nodes that end at the given offset.
 * @public
 */
export declare const findAtOffset: <T extends Element_2>(options: number | FindAtOffsetOptions, element: T) => T | undefined;

/**
 * @public
 */
export declare interface FindAtOffsetOptions {
    offset: number;
    includeRightBound?: boolean;
}

/**
 * Transforms data to an Element from a particular namespace.
 *
 * The name of the function was originally `from`,
 * but it was renamed to `fromFn` to avoid issues with Parcel.js:
 *
 * - https://github.com/parcel-bundler/parcel/issues/9473
 * - https://github.com/swagger-api/swagger-ui/issues/9466#issuecomment-1881053410
 * @public
 */
export declare const from: (data: any, namespace?: Namespace_2) => Element_2;

/**
 * @public
 */
export declare const getNodeType: <T extends Element_2>(element: T) => string | undefined;

/**
 * @public
 */
export declare const hasBasicElementProps: (element: unknown) => element is ElementBasicsTrait;

/**
 * @public
 */
export declare const hasClass: (cls: string, element: unknown) => element is ElementClassesTrait;

/**
 * @public
 */
export declare const hasElementSourceMap: <T extends Element_2>(element?: T) => boolean;

/**
 * @public
 */
export declare const hasMethod: <T extends string>(name: T, element: unknown) => element is { [key in T]: (...args: unknown[]) => unknown; };

/**
 * @public
 */
export declare class IdentityManager {
    protected readonly uuid: ShortUniqueId;
    protected readonly identityMap: WeakMap<Element_2, StringElement>;
    constructor({ length }?: {
        length?: number | undefined;
    });
    identify<T extends Element_2>(this: IdentityManager, element: T): StringElement;
    forget<T extends Element_2>(element: T): boolean;
    generateId(): string;
}

/**
 * @public
 */
export declare const includesClasses: <T extends Element_2>(classes: string[], element: T) => boolean;

/**
 * @public
 */
export declare const includesSymbols: <T extends Element_2>(symbols: string[], element: T) => boolean;

/**
 * @public
 */
export declare const isAnnotationElement: ElementPredicate<AnnotationElement>;

/**
 * @public
 */
export declare const isArrayElement: ElementPredicate<ArrayElement>;

/**
 * @public
 */
export declare const isBooleanElement: ElementPredicate<BooleanElement>;

/**
 * @public
 */
declare const isCommentElement: ElementPredicate<CommentElement>;

/**
 * @public
 */
export declare const isElement: ElementPredicate<Element_2>;

/**
 * @public
 */
export declare const isElementType: (name: string, element: unknown) => element is ElementTypeTrait;

/**
 * @public
 */
export declare const isLinkElement: ElementPredicate<LinkElement>;

/**
 * @public
 */
export declare const isMemberElement: ElementPredicate<MemberElement>;

/**
 * @public
 */
export declare const isNullElement: ElementPredicate<NullElement>;

/**
 * @public
 */
export declare const isNumberElement: ElementPredicate<NumberElement>;

/**
 * @public
 */
export declare const isObjectElement: ElementPredicate<ObjectElement>;

/**
 * @public
 */
export declare const isParseResultElement: ElementPredicate<ParseResultElement>;

/**
 * @public
 */
export declare const isPrimitiveElement: ElementPredicate<PrimitiveElement>;

/**
 * @public
 */
export declare const isRefElement: ElementPredicate<RefElement>;

/**
 * @public
 */
export declare const isStringElement: ElementPredicate<StringElement>;

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
};

export { KeyValuePair }

export { LinkElement }

/**
 * @public
 */
export declare class MediaTypes<T> extends Array<T> {
    unknownMediaType: string;
    filterByFormat(): void;
    findBy(): void;
    latest(): void;
}

export { MemberElement }

export { mergeAllVisitors }

/**
 * @public
 */
export declare const mergeLeft: typeof mergeRight;

/**
 * @public
 */
export declare const mergeRight: {
    (targetElement: ObjectOrArrayElement, sourceElement: ObjectOrArrayElement, options?: MergeRightOptions): AnyElement;
    all(list: ObjectOrArrayElement[], options?: MergeRightOptions): any;
};

/**
 * @public
 */
declare type MergeRightOptions = Omit<DeepMergeUserOptions, 'customMerge' | 'clone'>;
export { MergeRightOptions as MergeLeftOptions }
export { MergeRightOptions }

export { Meta }

/**
 * @public
 */
export declare class Namespace extends Namespace_2 {
    constructor();
}

/**
 * @public
 */
export declare const namespace: Namespace;

export { NamespacePluginOptions }

export { NullElement }

export { NumberElement }

export { ObjectElement }

/**
 * @public
 */
export declare type ObjectOrArrayElement = ObjectElement | ArrayElement;

export { ObjectSlice }

/**
 * Computes upwards edges from every child to its parent.
 * @public
 */
export declare const parents: <T extends Element_2>(element: T) => WeakMap<Element_2, Element_2 | undefined>;

/**
 * @public
 */
export declare class ParseResultElement extends ArrayElement {
    constructor(content?: Array<any>, meta?: Meta, attributes?: Attributes);
    get api(): Element_2 | undefined;
    get results(): ArraySlice;
    get result(): Element_2 | undefined;
    get annotations(): ArraySlice;
    get warnings(): ArraySlice;
    get errors(): ArraySlice;
    get isEmpty(): boolean;
    replaceResult(replacement: Element_2): boolean;
}

/**
 * @public
 */
export declare type PredicateCreator<T extends Element_2> = (helpers: PredicateHelpers) => ElementPredicate<T>;

/**
 * @public
 */
export declare interface PredicateHelpers {
    hasMethod: typeof hasMethod;
    hasBasicElementProps: typeof hasBasicElementProps;
    primitiveEq: typeof primitiveEq;
    isElementType: typeof isElementType;
    hasClass: typeof hasClass;
}

/**
 * @public
 */
export declare type Predicates = {
    isPrimitiveElement: typeof isPrimitiveElement;
};

/**
 * @public
 */
export declare type PrimitiveElement = ObjectElement | ArrayElement | BooleanElement | NumberElement | StringElement | NullElement | MemberElement;

/**
 * @public
 */
export declare const primitiveEq: (val: unknown, element: unknown) => element is ElementPrimitiveBehavior;

export { RefElement }

export { refract }

/**
 * Plugin for decorating every element in ApiDOM tree with UUID.
 * @public
 */
export declare const refractorPluginElementIdentity: ({ length }?: {
    length?: number | undefined;
}) => () => {
    pre(): void;
    visitor: {
        enter<T extends Element_2>(element: T): void;
    };
    post(): void;
};

/**
 * Plugin for decorating every semantic element in ApiDOM tree with UUID.
 * @public
 */
export declare const refractorPluginSemanticElementIdentity: ({ length }?: {
    length?: number | undefined;
}) => ({ predicates }: {
    predicates: Predicates;
}) => {
    pre(): void;
    visitor: {
        enter<T extends Element_2>(element: T): void;
    };
    post(): void;
};

/**
 * Complement of filter.
 * @public
 */
export declare const reject: <T extends Element_2>(predicate: (element: any) => boolean, element: T) => ArraySlice;

/**
 * Transforms ApiDOM into S-expressions (Symbolic Expressions).
 * @public
 */
export declare const sexprs: (element: Element_2) => string;

/**
 * @public
 */
export declare class ShallowCloneError extends CloneError {
}

/**
 * Tests whether at least one element passes the predicate.
 * @public
 */
export declare const some: <T extends Element_2>(predicate: (element: any) => boolean, element: T) => boolean;

declare type SourceMapContainer = {
    startPositionRow?: number;
    startPositionColumn?: number;
    startIndex?: number;
    endPositionRow?: number;
    endPositionColumn?: number;
    endIndex?: number;
    [key: string]: any;
};

export { StringElement }

/**
 * @public
 */
export declare const toJSON: (element: Element_2, replacer?: (this: any, key: string, value: any) => any, space?: string | number) => string;

/**
 * @public
 */
export declare interface Toolbox {
    predicates: typeof basePredicates;
    namespace: typeof namespace;
}

/**
 * Create a refracted string representation of an Element.
 * @public
 */
declare const toString_2: (element: Element_2, namespace?: Namespace_2) => string;
export { toString_2 as toString }

/**
 * @public
 */
export declare const toValue: <T extends Element_2 | unknown>(element: T) => any;

/**
 * @public
 */
export declare const toYAML: (element: Element_2, { directive }?: {
    directive?: boolean | undefined;
}) => string;

/**
 * This is a mutating function. If you don't want your Element to be mutated,
 * clone in before passing it to this function.
 * @public
 */
export declare const transclude: (search: Element_2, replace: Element_2, element: Element_2) => Element_2 | undefined;

/**
 * This is a mutating stamp. If you don't want your Element to be mutated,
 * clone in before passing it to initializer of this stamp.
 * @public
 */
export declare class Transcluder {
    protected element: Element_2;
    protected edges: WeakMap<Element_2, Element_2 | undefined>;
    constructor({ element }: {
        element: Element_2;
    });
    transclude(search: Element_2, replace: Element_2): Element_2 | undefined;
}

/**
 * Executes the callback on this element and all descendants.
 * @public
 */
export declare const traverse: <T extends Element_2>(options: Callback | TraverseOptions, element: T) => void;

/**
 * @public
 */
export declare interface TraverseOptions {
    callback?: Callback;
    predicate?: (element: any) => boolean;
}

/**
 * @public
 */
export declare const visit: (root: Element_2, visitor: any, { keyMap, ...rest }?: {
    keyMap?: {
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
    } | undefined;
}) => Element_2;

export { }
