import { Buffer } from 'buffer';
const base64Decoder = input => {
  return Buffer.from(input, 'base64').toString();
};
export default base64Decoder;