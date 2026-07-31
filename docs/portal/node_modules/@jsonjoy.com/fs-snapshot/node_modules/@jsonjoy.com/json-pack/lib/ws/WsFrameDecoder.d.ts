import { StreamingOctetReader } from '@jsonjoy.com/buffers/lib/StreamingOctetReader';
import { WsCloseFrame, WsFrameHeader } from './frames';
export declare class WsFrameDecoder {
    readonly reader: StreamingOctetReader;
    push(uint8: Uint8Array): void;
    readFrameHeader(): WsFrameHeader | undefined;
    /**
     * Read application data of a frame and copy it to the destination buffer.
     * Receives the frame header and the number of bytes that still need to be
     * copied, returns back the number of bytes that still need to be copied in
     * subsequent calls.
     *
     * @param frame Frame header.
     * @param remaining How many bytes are remaining to be copied.
     * @param dst The destination buffer to write to.
     * @param pos Position in the destination buffer to start writing to.
     * @returns The number of bytes that still need to be copied in the next call.
     */
    readFrameData(frame: WsFrameHeader, remaining: number, dst: Uint8Array, pos: number): number;
    copyFrameData(frame: WsFrameHeader, dst: Uint8Array, pos: number): void;
    /**
     * Reads application data of the CLOSE frame and sets the code and reason
     * properties of the frame.
     *
     * @param frame Close frame.
     */
    readCloseFrameData(frame: WsCloseFrame): void;
}
