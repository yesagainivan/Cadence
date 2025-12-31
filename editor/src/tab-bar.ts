/**
 * Tab Bar Component
 * 
 * Manages multiple open files with tabs.
 * Supports close, dirty indicator, and tab switching.
 */

export interface TabInfo {
    path: string;
    name: string;
    content: string;
    isDirty: boolean;
}

export interface TabBarCallbacks {
    onTabSelect: (path: string) => void;
    onTabClose: (path: string) => void;
    onSaveRequest: (path: string) => Promise<void>;
}

/**
 * Tab Bar Component
 */
export class TabBar {
    private container: HTMLElement;
    private callbacks: TabBarCallbacks;
    private tabs: Map<string, TabInfo> = new Map();
    private activeTab: string | null = null;

    constructor(container: HTMLElement, callbacks: TabBarCallbacks) {
        this.container = container;
        this.callbacks = callbacks;
        this.setupContainer();
    }

    private setupContainer(): void {
        this.container.className = 'tab-bar';
        this.render();
    }

    /**
     * Open a file in a new tab (or switch to existing)
     */
    openTab(path: string, content: string): void {
        if (this.tabs.has(path)) {
            // Tab already exists, just switch to it
            this.setActiveTab(path);
            return;
        }

        // Create new tab
        const name = path.split('/').pop() || path;
        this.tabs.set(path, {
            path,
            name,
            content,
            isDirty: false,
        });

        this.setActiveTab(path);
    }

    /**
     * Close a tab
     */
    closeTab(path: string): void {
        const tab = this.tabs.get(path);
        if (!tab) return;

        // Confirm if dirty
        if (tab.isDirty) {
            if (!confirm(`"${tab.name}" has unsaved changes. Close anyway?`)) {
                return;
            }
        }

        this.tabs.delete(path);

        // Switch to another tab if this was active
        if (this.activeTab === path) {
            const remaining = Array.from(this.tabs.keys());
            this.activeTab = remaining.length > 0 ? remaining[remaining.length - 1] : null;
            if (this.activeTab) {
                this.callbacks.onTabSelect(this.activeTab);
            }
        }

        this.callbacks.onTabClose(path);
        this.render();
    }

    /**
     * Set the active tab
     */
    setActiveTab(path: string): void {
        if (!this.tabs.has(path)) return;
        this.activeTab = path;
        this.callbacks.onTabSelect(path);
        this.render();
    }

    /**
     * Mark a tab as dirty (unsaved changes)
     */
    setDirty(path: string, isDirty: boolean): void {
        const tab = this.tabs.get(path);
        if (tab) {
            tab.isDirty = isDirty;
            this.render();
        }
    }

    /**
     * Update tab content (without triggering dirty)
     */
    updateContent(path: string, content: string): void {
        const tab = this.tabs.get(path);
        if (tab) {
            tab.content = content;
        }
    }

    /**
     * Get the content of a tab
     */
    getContent(path: string): string | null {
        return this.tabs.get(path)?.content ?? null;
    }

    /**
     * Get the active tab path
     */
    getActiveTab(): string | null {
        return this.activeTab;
    }

    /**
     * Get info about a tab
     */
    getTabInfo(path: string): TabInfo | undefined {
        return this.tabs.get(path);
    }

    /**
     * Check if a tab exists
     */
    hasTab(path: string): boolean {
        return this.tabs.has(path);
    }

    /**
     * Handle file rename
     */
    renameTab(oldPath: string, newPath: string): void {
        const tab = this.tabs.get(oldPath);
        if (tab) {
            this.tabs.delete(oldPath);
            tab.path = newPath;
            tab.name = newPath.split('/').pop() || newPath;
            this.tabs.set(newPath, tab);

            if (this.activeTab === oldPath) {
                this.activeTab = newPath;
            }

            this.render();
        }
    }

    /**
     * Handle file deletion
     */
    removeTab(path: string): void {
        if (this.tabs.has(path)) {
            this.tabs.delete(path);
            if (this.activeTab === path) {
                const remaining = Array.from(this.tabs.keys());
                this.activeTab = remaining.length > 0 ? remaining[remaining.length - 1] : null;
            }
            this.render();
        }
    }

    private render(): void {
        if (this.tabs.size === 0) {
            this.container.innerHTML = `
                <div class="tab-bar-empty">
                    No files open
                </div>
            `;
            return;
        }

        const tabsHtml = Array.from(this.tabs.values()).map(tab => `
            <div class="tab ${tab.path === this.activeTab ? 'active' : ''}" data-path="${tab.path}">
                <span class="tab-icon">🎵</span>
                <span class="tab-name">${tab.name}</span>
                ${tab.isDirty ? '<span class="tab-dirty">●</span>' : ''}
                <button class="tab-close" data-path="${tab.path}" title="Close">×</button>
            </div>
        `).join('');

        this.container.innerHTML = `
            <div class="tab-bar-tabs">${tabsHtml}</div>
        `;

        this.bindEvents();
    }

    private bindEvents(): void {
        // Tab click (switch)
        this.container.querySelectorAll('.tab').forEach(tab => {
            tab.addEventListener('click', (e) => {
                // Don't switch if clicking close button
                if ((e.target as HTMLElement).classList.contains('tab-close')) return;

                const path = (tab as HTMLElement).dataset.path;
                if (path) {
                    this.setActiveTab(path);
                }
            });

            // Middle-click to close
            tab.addEventListener('auxclick', (e) => {
                if ((e as MouseEvent).button === 1) {
                    const path = (tab as HTMLElement).dataset.path;
                    if (path) {
                        this.closeTab(path);
                    }
                }
            });
        });

        // Close button click
        this.container.querySelectorAll('.tab-close').forEach(btn => {
            btn.addEventListener('click', (e) => {
                e.stopPropagation();
                const path = (btn as HTMLElement).dataset.path;
                if (path) {
                    this.closeTab(path);
                }
            });
        });
    }
}
