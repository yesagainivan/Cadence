/**
 * File Tree Panel Component
 * 
 * Displays the virtual filesystem contents in a tree view.
 * Supports file/folder creation, deletion, and selection.
 */

import { getFileSystemService } from './filesystem-service.js';
import type { FileSystemEvent } from './filesystem-service.js';

export interface FileTreeCallbacks {
    onFileSelect: (path: string) => void;
    onFileCreate: (path: string) => void;
    onFileDelete: (path: string) => void;
    onFileRename: (oldPath: string, newPath: string) => void;
}

/**
 * File Tree Panel
 */
export class FileTreePanel {
    private container: HTMLElement;
    private callbacks: FileTreeCallbacks;
    private selectedPath: string | null = null;
    private expandedDirs: Set<string> = new Set(['/']);
    private unsubscribe: (() => void) | null = null;

    constructor(container: HTMLElement, callbacks: FileTreeCallbacks) {
        this.container = container;
        this.callbacks = callbacks;
        this.setupContainer();
        this.subscribeToChanges();
    }

    private setupContainer(): void {
        this.container.innerHTML = `
            <div class="file-tree-header">
                <span class="file-tree-title">Files</span>
                <div class="file-tree-actions">
                    <button class="file-tree-action" data-action="new-file" title="New File">
                        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                            <path d="M8 4a.5.5 0 0 1 .5.5v3h3a.5.5 0 0 1 0 1h-3v3a.5.5 0 0 1-1 0v-3h-3a.5.5 0 0 1 0-1h3v-3A.5.5 0 0 1 8 4z"/>
                        </svg>
                    </button>
                    <button class="file-tree-action" data-action="new-folder" title="New Folder">
                        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                            <path d="M.54 3.87.5 3a2 2 0 0 1 2-2h3.672a2 2 0 0 1 1.414.586l.828.828A2 2 0 0 0 9.828 3H12.5a2 2 0 0 1 2 2v1H.54z"/>
                            <path d="M14 6.5V13a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6.5h12zM8.5 10a.5.5 0 0 0-1 0v2a.5.5 0 0 0 1 0v-2zm1.5-.5a.5.5 0 0 1 .5.5v2a.5.5 0 0 1-1 0v-2a.5.5 0 0 1 .5-.5zm-4 .5a.5.5 0 0 0-1 0v2a.5.5 0 0 0 1 0v-2z"/>
                        </svg>
                    </button>
                    <button class="file-tree-action" data-action="refresh" title="Refresh">
                        <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                            <path fill-rule="evenodd" d="M8 3a5 5 0 1 0 4.546 2.914.5.5 0 0 1 .908-.417A6 6 0 1 1 8 2v1z"/>
                            <path d="M8 4.466V.534a.25.25 0 0 1 .41-.192l2.36 1.966c.12.1.12.284 0 .384L8.41 4.658A.25.25 0 0 1 8 4.466z"/>
                        </svg>
                    </button>
                </div>
            </div>
            <div class="file-tree-content"></div>
        `;

        // Bind action buttons
        this.container.querySelectorAll('.file-tree-action').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const action = (btn as HTMLElement).dataset.action;
                this.handleAction(action || '');
                e.stopPropagation();
            });
        });
    }

    private subscribeToChanges(): void {
        const fs = getFileSystemService();
        this.unsubscribe = fs.addListener((event: FileSystemEvent) => {
            console.log('[FileTree] FS event:', event);
            this.refresh();
        });
    }

    /**
     * Refresh the file tree
     */
    async refresh(): Promise<void> {
        const fs = getFileSystemService();
        const content = this.container.querySelector('.file-tree-content');
        if (!content) return;

        try {
            await fs.initialize();
            const items = await this.buildTree('/');
            content.innerHTML = items;
            this.bindTreeEvents();
        } catch (e) {
            content.innerHTML = `<div class="file-tree-error">Failed to load files: ${e}</div>`;
        }
    }

    private async buildTree(dirPath: string, depth = 0): Promise<string> {
        const fs = getFileSystemService();
        const entries = await fs.listDirectory(dirPath);

        if (entries.length === 0 && depth === 0) {
            return `<div class="file-tree-empty">
                No files yet.<br>
                <button class="file-tree-create-first">Create a file</button>
            </div>`;
        }

        let html = '';

        for (const entry of entries) {
            const isSelected = entry.path === this.selectedPath;
            const isExpanded = this.expandedDirs.has(entry.path);
            const indent = depth * 16;

            if (entry.isDirectory) {
                html += `
                    <div class="file-tree-item file-tree-folder ${isExpanded ? 'expanded' : ''}" 
                         data-path="${entry.path}" 
                         style="padding-left: ${indent + 8}px">
                        <span class="file-tree-icon folder-icon">${isExpanded ? '📂' : '📁'}</span>
                        <span class="file-tree-name">${entry.name}</span>
                    </div>
                `;

                if (isExpanded) {
                    html += await this.buildTree(entry.path, depth + 1);
                }
            } else {
                const isCadence = entry.name.endsWith('.cadence');
                const icon = isCadence ? '🎵' : '📄';

                html += `
                    <div class="file-tree-item file-tree-file ${isSelected ? 'selected' : ''}" 
                         data-path="${entry.path}"
                         style="padding-left: ${indent + 8}px">
                        <span class="file-tree-icon">${icon}</span>
                        <span class="file-tree-name">${entry.name}</span>
                    </div>
                `;
            }
        }

        return html;
    }

    private bindTreeEvents(): void {
        // File click
        this.container.querySelectorAll('.file-tree-file').forEach(item => {
            item.addEventListener('click', () => {
                const path = (item as HTMLElement).dataset.path;
                if (path) {
                    this.selectFile(path);
                }
            });

            // Context menu for files
            item.addEventListener('contextmenu', (e) => {
                e.preventDefault();
                const path = (item as HTMLElement).dataset.path;
                if (path) this.showContextMenu(e as MouseEvent, path, false);
            });
        });

        // Folder click (toggle expand)
        this.container.querySelectorAll('.file-tree-folder').forEach(item => {
            item.addEventListener('click', () => {
                const path = (item as HTMLElement).dataset.path;
                if (path) {
                    this.toggleFolder(path);
                }
            });

            // Context menu for folders
            item.addEventListener('contextmenu', (e) => {
                e.preventDefault();
                const path = (item as HTMLElement).dataset.path;
                if (path) this.showContextMenu(e as MouseEvent, path, true);
            });
        });

        // Create first file button
        const createFirst = this.container.querySelector('.file-tree-create-first');
        if (createFirst) {
            createFirst.addEventListener('click', () => this.handleAction('new-file'));
        }
    }

    private selectFile(path: string): void {
        this.selectedPath = path;
        this.callbacks.onFileSelect(path);
        this.refresh();
    }

    private toggleFolder(path: string): void {
        if (this.expandedDirs.has(path)) {
            this.expandedDirs.delete(path);
        } else {
            this.expandedDirs.add(path);
        }
        this.refresh();
    }

    private async handleAction(action: string): Promise<void> {
        const fs = getFileSystemService();

        switch (action) {
            case 'new-file': {
                const name = prompt('Enter file name:', 'untitled.cadence');
                if (name) {
                    const path = `/${name}`;
                    try {
                        await fs.writeFile(path, `// ${name}\n\n`);
                        this.callbacks.onFileCreate(path);
                        this.selectFile(path);
                    } catch (e) {
                        alert(`Failed to create file: ${e}`);
                    }
                }
                break;
            }
            case 'new-folder': {
                const name = prompt('Enter folder name:');
                if (name) {
                    const path = `/${name}`;
                    try {
                        await fs.createDirectory(path);
                        this.expandedDirs.add(path);
                        this.refresh();
                    } catch (e) {
                        alert(`Failed to create folder: ${e}`);
                    }
                }
                break;
            }
            case 'refresh':
                this.refresh();
                break;
        }
    }

    private showContextMenu(event: MouseEvent, path: string, isDir: boolean): void {
        // Remove any existing context menu
        document.querySelectorAll('.file-tree-context-menu').forEach(m => m.remove());

        const menu = document.createElement('div');
        menu.className = 'file-tree-context-menu';
        menu.style.left = `${event.clientX}px`;
        menu.style.top = `${event.clientY}px`;

        const items = isDir ? [
            { label: 'New File Here...', action: 'new-file-in' },
            { label: 'Delete Folder', action: 'delete' },
        ] : [
            { label: 'Rename...', action: 'rename' },
            { label: 'Delete', action: 'delete' },
        ];

        menu.innerHTML = items.map(item =>
            `<div class="context-menu-item" data-action="${item.action}">${item.label}</div>`
        ).join('');

        document.body.appendChild(menu);

        // Handle menu clicks
        menu.querySelectorAll('.context-menu-item').forEach(item => {
            item.addEventListener('click', async () => {
                const action = (item as HTMLElement).dataset.action;
                menu.remove();

                const fs = getFileSystemService();

                switch (action) {
                    case 'delete':
                        if (confirm(`Delete "${path}"?`)) {
                            try {
                                if (isDir) {
                                    await fs.deleteDirectory(path, true);
                                } else {
                                    await fs.deleteFile(path);
                                }
                                this.callbacks.onFileDelete(path);
                                if (this.selectedPath === path) {
                                    this.selectedPath = null;
                                }
                                this.refresh();
                            } catch (e) {
                                alert(`Failed to delete: ${e}`);
                            }
                        }
                        break;

                    case 'rename': {
                        const oldName = path.split('/').pop() || '';
                        const newName = prompt('New name:', oldName);
                        if (newName && newName !== oldName) {
                            const newPath = path.replace(/[^/]+$/, newName);
                            try {
                                await fs.rename(path, newPath);
                                this.callbacks.onFileRename(path, newPath);
                                if (this.selectedPath === path) {
                                    this.selectedPath = newPath;
                                }
                                this.refresh();
                            } catch (e) {
                                alert(`Failed to rename: ${e}`);
                            }
                        }
                        break;
                    }

                    case 'new-file-in': {
                        const name = prompt('Enter file name:', 'untitled.cadence');
                        if (name) {
                            const newPath = `${path}/${name}`;
                            try {
                                await fs.writeFile(newPath, `// ${name}\n\n`);
                                this.expandedDirs.add(path);
                                this.callbacks.onFileCreate(newPath);
                                this.selectFile(newPath);
                            } catch (e) {
                                alert(`Failed to create file: ${e}`);
                            }
                        }
                        break;
                    }
                }
            });
        });

        // Close on outside click
        const closeMenu = (e: MouseEvent) => {
            if (!menu.contains(e.target as Node)) {
                menu.remove();
                document.removeEventListener('click', closeMenu);
            }
        };
        setTimeout(() => document.addEventListener('click', closeMenu), 0);
    }

    /**
     * Clean up event listeners
     */
    destroy(): void {
        if (this.unsubscribe) {
            this.unsubscribe();
        }
    }
}
