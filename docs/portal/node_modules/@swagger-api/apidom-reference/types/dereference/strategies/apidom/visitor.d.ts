import { Element, RefElement } from '@swagger-api/apidom-core';
import Reference from '../../../Reference.ts';
import type { ReferenceOptions } from '../../../options/index.ts';
/**
 * The following rules apply:
 *
 * 1. When referencing an element in the local document, the id of the element MAY be used
 * 2. When referencing remote elements, an absolute URL or relative URL MAY be used
 * 3. When a URL fragment exists in the URL given, it references the element with the matching id in the given document. The URL fragment MAY need to be URL decoded before making a match.
 * 4. When a URL fragment does not exist, the URL references the root element
 * 5. When path is used, it references the given property of the referenced element
 * 6. When path is used in an element that includes the data of the pointer (such as with ref), the referenced path MAY need to be converted to a refract structure in order to be valid
 *
 * WARNING: this implementation only supports referencing elements in the local document. Points 2-4 are not supported.
 * @public
 */
export interface ApiDOMDereferenceVisitorOptions {
    readonly reference: Reference;
    readonly options: ReferenceOptions;
}
/**
 * @public
 */
declare class ApiDOMDereferenceVisitor {
    protected readonly reference: Reference;
    protected readonly options: ReferenceOptions;
    constructor({ reference, options }: ApiDOMDereferenceVisitorOptions);
    protected handleDereferenceError(error: unknown, refEl: Element): undefined;
    protected toBaseURI(uri: string): string;
    protected toReference(uri: string): Promise<Reference>;
    RefElement(refElement: RefElement, key: string | number, parent: Element | undefined, path: (string | number)[], ancestors: [Element | Element[]]): Promise<unknown>;
}
export default ApiDOMDereferenceVisitor;
