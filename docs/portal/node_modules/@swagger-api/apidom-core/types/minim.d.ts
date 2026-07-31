/* eslint-disable spaced-comment, max-classes-per-file */
/// <reference types="node" />

declare module 'minim' {
  export type Meta = Record<string, any>;
  export type Attributes = Record<string, any>;
  export type Predicate = (element: Element) => boolean;
  type Callback = (element: Element) => void;

  export function refract(value: any): Element;

  export class Element {
    static refract(value: any, options?: Record<string, any>): Element;

    public element: string;

    public meta: ObjectElement;

    public attributes: ObjectElement;

    public classes: ArrayElement;

    public children: ArraySlice;

    public parent: Element;

    public content: Array<unknown>;

    constructor(content?: Array<unknown>, meta?: Meta, attributes?: Attributes);

    set id(element: Element);

    get id(): Element;

    equals(value: any): boolean;

    toValue(): any;

    getMetaProperty(name: string, value?: any): any;

    setMetaProperty(name: string, value: any): void;

    freeze(): void;

    clone(): Element;

    primitive(): string | undefined;

    [key: string]: unknown;
  }

  interface Type<T> extends Element {
    new (...args: any[]): T;
  }

  type ExtendingElement<T extends Element = Element> = Record<string, T>;

  export class Namespace {
    get elements(): Record<string, Element>;

    toRefract(element: Element): JSON;

    toElement(value: any): Element;

    fromRefract(doc: Record<string, any> | Array<any>): Element;

    register(name: string, elementClass: any): Namespace;

    detect(test: any, elementClass: any, givenPrepend: boolean): Namespace;

    use(plugin: NamespacePlugin): Namespace;

    getElementClass(element: string): typeof Element;
  }

  export interface NamespacePluginOptions {
    base: Namespace;
  }

  export interface NamespacePlugin {
    namespace(options: NamespacePluginOptions): Namespace;
  }

  export class StringElement extends Element {
    constructor(content?: string, meta?: Meta, attributes?: Attributes);
  }

  export class NumberElement extends Element {
    constructor(content?: number, meta?: Meta, attributes?: Attributes);
  }

  export class NullElement extends Element {}

  export class BooleanElement extends Element {
    constructor(content?: boolean, meta?: Meta, attributes?: Attributes);
  }

  export class ArrayElement extends Element {
    constructor(content?: unknown[], meta?: Meta, attributes?: Attributes);

    first: Element | undefined;

    second: Element | undefined;

    get(index: string | number): any;

    set(index: string | number, element: Element): void;

    find(predicate: Predicate): ArraySlice;

    findElements(condition: Predicate, givenOptions: any): Array<Element>;

    filter(predicate: Predicate): ArraySlice;

    contains(value: any): boolean;

    includes(value: any): boolean;

    push(value: any): ArrayElement;

    concat(value: ArrayElement): ArrayElement;

    ['fantasy-land/map'](transform: any): ArrayElement;

    forEach(callback: (item: Element, index: number) => void, thisArg?: unknown): void;

    [Symbol.iterator](): IterableIterator<any>;

    get length(): number;

    map(callback: (element: Element) => Element, thisArg?: unknown): Array<Element>;
  }

  export class ObjectElement extends ArrayElement {
    constructor(content?: Record<string, unknown>, meta?: Meta, attributes?: Attributes);

    remove(name: string): Element;

    set(key: string | StringElement | number, value: any): ObjectElement;

    hasKey(value: string): boolean;

    getKey(value: string): StringElement;

    getMember(key: string): MemberElement;

    // @ts-ignore
    forEach(
      callback: (value: Element, key: Element, item: Element) => void,
      thisArg?: unknown,
    ): void;

    map(
      callback: (value: Element, key: Element, member: MemberElement) => MemberElement,
      thisArg?: unknown,
    ): Array<MemberElement>;

    keys(): unknown[];
  }

  export class MemberElement extends Element {
    constructor(key?: unknown, value?: unknown, meta?: Meta, attributes?: Attributes);

    get key(): unknown;

    set key(key: unknown);

    get value(): unknown;

    set value(value: unknown);
  }

  export class LinkElement extends Element {
    constructor(content?: unknown, meta?: Meta, attributes?: Attributes);

    get relation(): string;

    set relation(relation: string);

    get href(): string;

    set href(key: string);
  }

  export class RefElement extends Element {
    constructor(content?: unknown, meta?: Meta, attributes?: Attributes);

    get path(): string;

    set path(path: string);
  }

  export class ArraySlice {
    constructor(elements?: Array<unknown>);

    get length(): number;

    get first(): Element;

    get isEmpty(): boolean;

    filter(predicate: Predicate, thisArg?: unknown): ArraySlice;

    reject(predicate: Predicate, thisArg?: unknown): ArraySlice;

    forEach(callback: Callback): void;

    reduce<T>(callback: (acc: T, cur: Element) => T, initialValue: T): ArraySlice;

    map(callback: (currentValue: any, index: number) => any, thisArg?: unknown): ArraySlice;

    hasKey(value: string): boolean;

    get<T extends Element>(index: number): T;

    [Symbol.iterator](): IterableIterator<any>;

    toValue(): any;

    clone(): ArraySlice;
  }

  export class ObjectSlice extends ArraySlice {
    clone(): ObjectSlice;
  }

  export class KeyValuePair {
    public key: Element | undefined;

    public value: Element | undefined;

    constructor(key?: Element, value?: Element);

    clone(): KeyValuePair;
  }
}
