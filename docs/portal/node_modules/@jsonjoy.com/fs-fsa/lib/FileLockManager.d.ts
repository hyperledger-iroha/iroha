export declare class FileLockManager {
    private locks;
    acquireLock(path: string): boolean;
    releaseLock(path: string): void;
    isLocked(path: string): boolean;
    clear(): void;
}
