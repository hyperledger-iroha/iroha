import { MsgPackEncoderFast } from './MsgPackEncoderFast';
/**
 * @category Encoder
 */
export declare class MsgPackEncoderStable extends MsgPackEncoderFast {
    writeObj(obj: Record<string, unknown>): void;
}
