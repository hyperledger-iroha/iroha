const escape = referenceToken => {
  if (typeof referenceToken !== 'string' && typeof referenceToken !== 'number') {
    throw new TypeError('Reference token must be a string or number');
  }
  return String(referenceToken).replace(/~/g, '~0').replace(/\//g, '~1');
};
export default escape;