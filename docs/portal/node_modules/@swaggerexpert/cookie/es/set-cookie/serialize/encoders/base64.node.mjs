import { Buffer } from 'buffer';
const base64Encoder = input => {
  return Buffer.from(input).toString('base64');
};
export default base64Encoder;