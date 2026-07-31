import { propEq } from 'ramda';
import { ApiDOMError } from '@swagger-api/apidom-error';
import { isElement, isMemberElement, isArrayElement, isObjectElement, isRefElement, toValue, refract, visit, cloneDeep } from '@swagger-api/apidom-core';
import { URIFragmentIdentifier } from '@swagger-api/apidom-json-pointer/modern';
import MaximumResolveDepthError from "../../../errors/MaximumResolveDepthError.mjs";
import * as url from "../../../util/url.mjs";
import parse from "../../../parse/index.mjs";
import Reference from "../../../Reference.mjs";
import { evaluate } from "./selectors/element-id.mjs";
// @ts-ignore
const visitAsync = visit[Symbol.for('nodejs.util.promisify.custom')];

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

/**
 * @public
 */
class ApiDOMDereferenceVisitor {
  reference;
  options;
  constructor({
    reference,
    options
  }) {
    this.reference = reference;
    this.options = options;
  }
  handleDereferenceError(error, refEl) {
    var _this$options$derefer;
    if ((_this$options$derefer = this.options.dereference.dereferenceOpts) !== null && _this$options$derefer !== void 0 && _this$options$derefer.continueOnError) {
      var _this$options$derefer2;
      (_this$options$derefer2 = this.options.dereference.dereferenceOpts) === null || _this$options$derefer2 === void 0 || _this$options$derefer2.errors.push({
        error,
        refEl
      });
      return undefined;
    }
    throw error;
  }
  toBaseURI(uri) {
    return url.resolve(this.reference.uri, url.sanitize(url.stripHash(uri)));
  }
  async toReference(uri) {
    // detect maximum depth of resolution
    if (this.reference.depth >= this.options.resolve.maxDepth) {
      throw new MaximumResolveDepthError(`Maximum resolution depth of ${this.options.resolve.maxDepth} has been exceeded by file "${this.reference.uri}"`);
    }
    const baseURI = this.toBaseURI(uri);
    const {
      refSet
    } = this.reference;

    // we've already processed this Reference in past
    if (refSet.has(baseURI)) {
      return refSet.find(propEq(baseURI, 'uri'));
    }
    const parseResult = await parse(url.unsanitize(baseURI), {
      ...this.options,
      parse: {
        ...this.options.parse,
        mediaType: 'text/plain'
      }
    });

    // register new mutable reference with a refSet
    const mutableReference = new Reference({
      uri: baseURI,
      value: cloneDeep(parseResult),
      depth: this.reference.depth + 1
    });
    refSet.add(mutableReference);
    if (this.options.dereference.immutable) {
      // register new immutable reference with a refSet
      const immutableReference = new Reference({
        uri: `immutable://${baseURI}`,
        value: parseResult,
        depth: this.reference.depth + 1
      });
      refSet.add(immutableReference);
    }
    return mutableReference;
  }
  async RefElement(refElement, key, parent, path, ancestors) {
    const refURI = toValue(refElement);
    const refNormalizedURI = refURI.includes('#') ? refURI : `#${refURI}`;
    const retrievalURI = this.toBaseURI(refNormalizedURI);
    const isInternalReference = url.stripHash(this.reference.uri) === retrievalURI;
    const isExternalReference = !isInternalReference;

    // ignore resolving internal RefElements
    if (!this.options.resolve.internal && isInternalReference) {
      // skip traversing this ref element
      return false;
    }
    // ignore resolving external RefElements
    if (!this.options.resolve.external && isExternalReference) {
      // skip traversing this ref element
      return false;
    }
    let reference;
    try {
      reference = await this.toReference(refNormalizedURI);
    } catch (error) {
      return this.handleDereferenceError(error, refElement);
    }
    const refBaseURI = url.resolve(retrievalURI, refNormalizedURI);
    const elementID = URIFragmentIdentifier.fromURIReference(refBaseURI);
    let referencedElement;
    try {
      referencedElement = evaluate(elementID, reference.value.result);
    } catch (error) {
      return this.handleDereferenceError(error, refElement);
    }
    if (!isElement(referencedElement)) {
      const error = new ApiDOMError(`Referenced element with id="${elementID}" was not found`);
      return this.handleDereferenceError(error, refElement);
    }
    if (refElement === referencedElement) {
      const error = new ApiDOMError('RefElement cannot reference itself');
      return this.handleDereferenceError(error, refElement);
    }
    if (isRefElement(referencedElement)) {
      const error = new ApiDOMError('RefElement cannot reference another RefElement');
      return this.handleDereferenceError(error, refElement);
    }
    if (isExternalReference) {
      // dive deep into the fragment
      const visitor = new ApiDOMDereferenceVisitor({
        reference,
        options: this.options
      });
      referencedElement = await visitAsync(referencedElement, visitor);
    }

    /**
     * When path is used, it references the given property of the referenced element
     */
    const referencedElementPath = toValue(refElement.path);
    if (referencedElementPath !== 'element' && isElement(referencedElement)) {
      referencedElement = refract(referencedElement[referencedElementPath]);
    }

    /**
     * Transclusion of a Ref Element SHALL be defined in the if/else block below.
     */
    if (isObjectElement(referencedElement) && isObjectElement(ancestors[ancestors.length - 1]) && Array.isArray(parent) && typeof key === 'number') {
      /**
       * If the Ref Element is held by an Object Element and references an Object Element,
       * its content entries SHALL be inserted in place of the Ref Element.
       */
      parent.splice(key, 1, ...referencedElement.content);
    } else if (isArrayElement(referencedElement) && Array.isArray(parent) && typeof key === 'number') {
      /**
       * If the Ref Element is held by an Array Element and references an Array Element,
       * its content entries SHALL be inserted in place of the Ref Element.
       */
      parent.splice(key, 1, ...referencedElement.content);
    } else if (isMemberElement(parent)) {
      /**
       * The Ref Element is substituted by the Element it references.
       */
      parent.value = referencedElement; // eslint-disable-line no-param-reassign
    } else if (Array.isArray(parent)) {
      /**
       * The Ref Element is substituted by the Element it references.
       */
      parent[key] = referencedElement; // eslint-disable-line no-param-reassign
    }
    return !parent ? referencedElement : false;
  }
}
export default ApiDOMDereferenceVisitor;