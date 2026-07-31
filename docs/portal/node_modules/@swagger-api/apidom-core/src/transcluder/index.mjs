import Transcluder from "./Transcluder.mjs";
/**
 * This is a mutating function. If you don't want your Element to be mutated,
 * clone in before passing it to this function.
 * @public
 */
export const transclude = (search, replace, element) => {
  const transcluder = new Transcluder({
    element
  });
  return transcluder.transclude(search, replace);
};
export default Transcluder;