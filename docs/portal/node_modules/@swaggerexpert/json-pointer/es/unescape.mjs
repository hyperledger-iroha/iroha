const unescape = referenceToken => {
  if (typeof referenceToken !== 'string') {
    throw new TypeError('Reference token must be a string');
  }
  return referenceToken.replace(/~1/g, '/').replace(/~0/g, '~');
};
export default unescape;