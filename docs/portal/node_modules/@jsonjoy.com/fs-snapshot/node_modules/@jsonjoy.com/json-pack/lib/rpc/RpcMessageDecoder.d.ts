import { Reader } from '@jsonjoy.com/buffers/lib/Reader';
import { type RpcMessage } from './messages';
export declare class RpcMessageDecoder {
    decodeMessage(reader: Reader): RpcMessage | undefined;
    private readOpaqueAuth;
}
