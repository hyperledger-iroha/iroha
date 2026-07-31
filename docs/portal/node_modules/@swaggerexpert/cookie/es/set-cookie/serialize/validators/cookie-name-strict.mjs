import testCookieName from "../../../cookie/test/cookie-name.mjs";
const cookieNameStrictValidator = cookieName => {
  if (!testCookieName(cookieName)) {
    throw new TypeError(`Invalid cookie name: ${cookieName}`);
  }
};
export default cookieNameStrictValidator;