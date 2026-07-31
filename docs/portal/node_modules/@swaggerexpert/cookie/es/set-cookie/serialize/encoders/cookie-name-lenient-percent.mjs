import testCookieName from "../../../cookie/test/cookie-name.mjs";
import { percentEncodeChar } from "../../../utils.mjs";
const cookieNameLenientPercentEncoder = cookieName => {
  const value = String(cookieName);
  let result = '';
  for (const char of value) {
    result += testCookieName(char, {
      strict: false
    }) ? char : percentEncodeChar(char);
  }
  return result;
};
export default cookieNameLenientPercentEncoder;