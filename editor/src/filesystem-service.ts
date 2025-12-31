/**
 * Virtual Filesystem Service using Origin Private File System (OPFS)
 * 
 * Provides a persistent, sandboxed filesystem for storing Cadence project files.
 * OPFS is 2-4x faster than IndexedDB and provides a native filesystem API.
 */

// File info for listing
export interface FileInfo {
    name: string;
    path: string;
    isDirectory: boolean;
    size?: number;
    lastModified?: number;
}

// Event types for file system changes
export type FileSystemEvent =
    | { type: 'created'; path: string }
    | { type: 'modified'; path: string }
    | { type: 'deleted'; path: string }
    | { type: 'renamed'; oldPath: string; newPath: string };

export type FileSystemListener = (event: FileSystemEvent) => void;

/**
 * FileSystemService - OPFS-based virtual filesystem
 */
export class FileSystemService {
    private root: FileSystemDirectoryHandle | null = null;
    private initialized = false;
    private listeners: Set<FileSystemListener> = new Set();

    /**
     * Initialize the filesystem - must be called before any operations
     */
    async initialize(): Promise<void> {
        if (this.initialized) return;

        try {
            // Get the OPFS root directory
            this.root = await navigator.storage.getDirectory();
            this.initialized = true;
            console.log('[FileSystem] Initialized OPFS root');
        } catch (error) {
            console.error('[FileSystem] Failed to initialize OPFS:', error);
            throw new Error('Failed to initialize file system. OPFS may not be supported in this browser.');
        }
    }

    /**
     * Ensure the filesystem is initialized
     */
    private ensureInitialized(): FileSystemDirectoryHandle {
        if (!this.root || !this.initialized) {
            throw new Error('FileSystemService not initialized. Call initialize() first.');
        }
        return this.root;
    }

    /**
     * Get a directory handle, creating parent directories as needed
     */
    private async getDirectoryHandle(
        path: string,
        options: { create?: boolean } = {}
    ): Promise<FileSystemDirectoryHandle> {
        const root = this.ensureInitialized();

        // Normalize path
        const normalizedPath = this.normalizePath(path);
        if (normalizedPath === '' || normalizedPath === '/') {
            return root;
        }

        // Split into parts and traverse
        const parts = normalizedPath.split('/').filter(p => p.length > 0);
        let current = root;

        for (const part of parts) {
            current = await current.getDirectoryHandle(part, { create: options.create });
        }

        return current;
    }

    /**
     * Get a file handle at the given path
     */
    private async getFileHandle(
        path: string,
        options: { create?: boolean } = {}
    ): Promise<FileSystemFileHandle> {
        const normalizedPath = this.normalizePath(path);
        const parts = normalizedPath.split('/').filter(p => p.length > 0);

        if (parts.length === 0) {
            throw new Error('Invalid file path');
        }

        const fileName = parts.pop()!;
        const dirPath = parts.join('/');

        const dir = await this.getDirectoryHandle(dirPath, { create: options.create });
        return dir.getFileHandle(fileName, { create: options.create });
    }

    /**
     * Normalize a path (remove leading/trailing slashes, handle ..)
     */
    private normalizePath(path: string): string {
        // Remove leading and trailing slashes
        let normalized = path.replace(/^\/+|\/+$/g, '');

        // Handle . and .. 
        const parts = normalized.split('/');
        const result: string[] = [];

        for (const part of parts) {
            if (part === '.' || part === '') continue;
            if (part === '..') {
                result.pop();
            } else {
                result.push(part);
            }
        }

        return result.join('/');
    }

    /**
     * Read a file as text
     */
    async readFile(path: string): Promise<string> {
        try {
            const fileHandle = await this.getFileHandle(path);
            const file = await fileHandle.getFile();
            return await file.text();
        } catch (error) {
            throw new Error(`Failed to read file '${path}': ${error}`);
        }
    }

    /**
     * Write content to a file (creates file and parent directories if needed)
     */
    async writeFile(path: string, content: string): Promise<void> {
        try {
            const fileHandle = await this.getFileHandle(path, { create: true });
            const writable = await fileHandle.createWritable();
            await writable.write(content);
            await writable.close();

            // Notify listeners
            this.emit({ type: 'modified', path });
        } catch (error) {
            throw new Error(`Failed to write file '${path}': ${error}`);
        }
    }

    /**
     * Check if a file or directory exists
     */
    async exists(path: string): Promise<boolean> {
        try {
            const normalizedPath = this.normalizePath(path);
            const parts = normalizedPath.split('/').filter(p => p.length > 0);

            if (parts.length === 0) return true; // Root always exists

            const name = parts.pop()!;
            const dirPath = parts.join('/');

            const dir = await this.getDirectoryHandle(dirPath);

            // Try to get as file first, then directory
            try {
                await dir.getFileHandle(name);
                return true;
            } catch {
                try {
                    await dir.getDirectoryHandle(name);
                    return true;
                } catch {
                    return false;
                }
            }
        } catch {
            return false;
        }
    }

    /**
     * Delete a file
     */
    async deleteFile(path: string): Promise<void> {
        try {
            const normalizedPath = this.normalizePath(path);
            const parts = normalizedPath.split('/').filter(p => p.length > 0);

            if (parts.length === 0) {
                throw new Error('Cannot delete root directory');
            }

            const name = parts.pop()!;
            const dirPath = parts.join('/');

            const dir = await this.getDirectoryHandle(dirPath);
            await dir.removeEntry(name);

            this.emit({ type: 'deleted', path });
        } catch (error) {
            throw new Error(`Failed to delete '${path}': ${error}`);
        }
    }

    /**
     * Delete a directory (must be empty unless recursive)
     */
    async deleteDirectory(path: string, recursive = false): Promise<void> {
        try {
            const normalizedPath = this.normalizePath(path);
            const parts = normalizedPath.split('/').filter(p => p.length > 0);

            if (parts.length === 0) {
                throw new Error('Cannot delete root directory');
            }

            const name = parts.pop()!;
            const dirPath = parts.join('/');

            const dir = await this.getDirectoryHandle(dirPath);
            await dir.removeEntry(name, { recursive });

            this.emit({ type: 'deleted', path });
        } catch (error) {
            throw new Error(`Failed to delete directory '${path}': ${error}`);
        }
    }

    /**
     * Create a directory (and parent directories if needed)
     */
    async createDirectory(path: string): Promise<void> {
        try {
            await this.getDirectoryHandle(path, { create: true });
            this.emit({ type: 'created', path });
        } catch (error) {
            throw new Error(`Failed to create directory '${path}': ${error}`);
        }
    }

    /**
     * List contents of a directory
     */
    async listDirectory(path: string = '/'): Promise<FileInfo[]> {
        try {
            const dir = await this.getDirectoryHandle(path);
            const entries: FileInfo[] = [];

            // @ts-ignore - entries() is available in modern browsers
            for await (const [name, handle] of dir.entries()) {
                const isDirectory = handle.kind === 'directory';
                const fullPath = path === '/' ? `/${name}` : `${path}/${name}`;

                const info: FileInfo = {
                    name,
                    path: fullPath,
                    isDirectory,
                };

                // Get file-specific info
                if (!isDirectory) {
                    try {
                        const file = await (handle as FileSystemFileHandle).getFile();
                        info.size = file.size;
                        info.lastModified = file.lastModified;
                    } catch {
                        // Ignore errors getting file info
                    }
                }

                entries.push(info);
            }

            // Sort: directories first, then alphabetically
            entries.sort((a, b) => {
                if (a.isDirectory !== b.isDirectory) {
                    return a.isDirectory ? -1 : 1;
                }
                return a.name.localeCompare(b.name);
            });

            return entries;
        } catch (error) {
            throw new Error(`Failed to list directory '${path}': ${error}`);
        }
    }

    /**
     * Recursively list all files (not directories)
     */
    async listAllFiles(path: string = '/'): Promise<string[]> {
        const result: string[] = [];

        const processDir = async (dirPath: string) => {
            const entries = await this.listDirectory(dirPath);
            for (const entry of entries) {
                if (entry.isDirectory) {
                    await processDir(entry.path);
                } else {
                    result.push(entry.path);
                }
            }
        };

        await processDir(path);
        return result;
    }

    /**
     * Rename/move a file or directory
     */
    async rename(oldPath: string, newPath: string): Promise<void> {
        // OPFS doesn't support direct rename, so we copy then delete
        const isDir = (await this.listDirectory(this.normalizePath(oldPath).split('/').slice(0, -1).join('/') || '/'))
            .find(f => f.path === oldPath)?.isDirectory;

        if (isDir) {
            throw new Error('Directory rename not yet supported');
        }

        const content = await this.readFile(oldPath);
        await this.writeFile(newPath, content);
        await this.deleteFile(oldPath);

        this.emit({ type: 'renamed', oldPath, newPath });
    }

    /**
     * Add a listener for filesystem events
     */
    addListener(listener: FileSystemListener): () => void {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }

    /**
     * Emit an event to all listeners
     */
    private emit(event: FileSystemEvent): void {
        for (const listener of this.listeners) {
            try {
                listener(event);
            } catch (error) {
                console.error('[FileSystem] Listener error:', error);
            }
        }
    }

    /**
     * Check if OPFS is supported in this browser
     */
    static isSupported(): boolean {
        return 'storage' in navigator && 'getDirectory' in navigator.storage;
    }
}

// Singleton instance
let instance: FileSystemService | null = null;

/**
 * Get or create the FileSystemService singleton
 */
export function getFileSystemService(): FileSystemService {
    if (!instance) {
        instance = new FileSystemService();
    }
    return instance;
}

/**
 * Initialize the filesystem service (call once at app startup)
 */
export async function initializeFileSystem(): Promise<FileSystemService> {
    const service = getFileSystemService();
    await service.initialize();
    return service;
}
