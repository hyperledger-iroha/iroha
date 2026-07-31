import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers';
export declare class RmRecordEncoder<W extends IWriter & IWriterGrowable = IWriter & IWriterGrowable> {
    readonly writer: W;
    constructor(writer?: W);
    encodeHdr(fin: 0 | 1, length: number): Uint8Array;
    encodeRecord(record: Uint8Array): Uint8Array;
    writeHdr(fin: 0 | 1, length: number): void;
    writeRecord(record: Uint8Array): void;
    writeFragment(record: Uint8Array, offset: number, length: number, fin: 0 | 1): void;
    /**
     * To write an RM record in one pass this method reserves space for the RM
     * header, and returns the state, which needs to passed to `endRmRecord` to
     * finalize the RM header.
     */
    startRecord(): number;
    /**
     * Finalize the RM header started by `startRmRecord`.
     *
     * @param rmHeaderPosition The position returned by `startRmRecord`
     * @remarks This method will check if the data written after `startRmRecord`
     * fits into a single RM frame. If it does, it will write the RM header in
     * place. If it doesn't, it will move the data to a new location and write
     * it as multiple RM frames.
     */
    endRecord(rmHeaderPosition: number): void;
}
