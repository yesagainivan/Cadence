/**
 * ResizeHandle Component
 * 
 * A draggable divider that allows resizing adjacent panels.
 */

export type ResizeDirection = 'horizontal' | 'vertical';

export interface ResizeHandleOptions {
    direction: ResizeDirection;
    onResize: (delta: number) => void;
    onResizeEnd?: () => void;
}

/**
 * Creates a resize handle element and manages drag behavior
 */
export class ResizeHandle {
    private element: HTMLElement;
    private direction: ResizeDirection;
    private onResize: (delta: number) => void;
    private onResizeEnd?: () => void;
    private isDragging = false;
    private startPos = 0;

    constructor(options: ResizeHandleOptions) {
        this.direction = options.direction;
        this.onResize = options.onResize;
        this.onResizeEnd = options.onResizeEnd;

        this.element = document.createElement('div');
        this.element.className = `resize-handle resize-handle-${options.direction === 'horizontal' ? 'h' : 'v'}`;

        this.setupEventListeners();
    }

    /**
     * Get the DOM element
     */
    getElement(): HTMLElement {
        return this.element;
    }

    /**
     * Insert the handle between two elements
     */
    insertBetween(before: HTMLElement, after: HTMLElement): void {
        before.parentNode?.insertBefore(this.element, after);
    }

    private setupEventListeners(): void {
        this.element.addEventListener('mousedown', this.handleMouseDown.bind(this));

        // Use document-level listeners for move/up to handle cursor leaving the handle
        document.addEventListener('mousemove', this.handleMouseMove.bind(this));
        document.addEventListener('mouseup', this.handleMouseUp.bind(this));
    }

    private handleMouseDown(e: MouseEvent): void {
        e.preventDefault();
        this.isDragging = true;
        this.startPos = this.direction === 'horizontal' ? e.clientY : e.clientX;

        // Prevent text selection during drag
        document.body.style.userSelect = 'none';
        document.body.style.cursor = this.direction === 'horizontal' ? 'row-resize' : 'col-resize';

        this.element.classList.add('active');
    }

    private handleMouseMove(e: MouseEvent): void {
        if (!this.isDragging) return;

        const currentPos = this.direction === 'horizontal' ? e.clientY : e.clientX;
        const delta = currentPos - this.startPos;

        if (delta !== 0) {
            this.onResize(delta);
            this.startPos = currentPos;
        }
    }

    private handleMouseUp(): void {
        if (!this.isDragging) return;

        this.isDragging = false;
        document.body.style.userSelect = '';
        document.body.style.cursor = '';

        this.element.classList.remove('active');
        this.onResizeEnd?.();
    }

    /**
     * Clean up event listeners
     */
    destroy(): void {
        document.removeEventListener('mousemove', this.handleMouseMove.bind(this));
        document.removeEventListener('mouseup', this.handleMouseUp.bind(this));
        this.element.remove();
    }
}

// =============================================================================
// Helper: Create a resizable panel pair
// =============================================================================

export interface ResizablePanelConfig {
    container: HTMLElement;
    firstPanel: HTMLElement;
    secondPanel: HTMLElement;
    direction: ResizeDirection;
    storageKey?: string;
    minFirstSize?: number;
    minSecondSize?: number;
    defaultSize?: number;
    resizeSecond?: boolean;  // If true, resize secondPanel instead of firstPanel
}

/**
 * Sets up resizable panels with persistence
 */
export function setupResizablePanels(config: ResizablePanelConfig): ResizeHandle {
    const {
        firstPanel,
        secondPanel,
        direction,
        storageKey,
        minFirstSize = 100,
        minSecondSize = 100,
        defaultSize,
        resizeSecond = false,
    } = config;

    const targetPanel = resizeSecond ? secondPanel : firstPanel;

    // Load saved size or use default
    let targetSize = defaultSize;
    if (storageKey) {
        const saved = localStorage.getItem(storageKey);
        if (saved) {
            targetSize = parseInt(saved, 10);
        }
    }

    // Apply initial size
    if (targetSize !== undefined) {
        const prop = direction === 'horizontal' ? 'height' : 'width';
        targetPanel.style[prop] = `${targetSize}px`;
        targetPanel.style.flex = 'none';
    }

    // Create resize handle
    const handle = new ResizeHandle({
        direction,
        onResize: (delta) => {
            const prop = direction === 'horizontal' ? 'height' : 'width';
            const currentSize = targetPanel.getBoundingClientRect()[prop];
            const containerSize = config.container.getBoundingClientRect()[prop];

            // Invert delta if resizing second panel
            const adjustedDelta = resizeSecond ? -delta : delta;
            let newSize = currentSize + adjustedDelta;

            // Apply min constraints
            const minTarget = resizeSecond ? minSecondSize : minFirstSize;
            const minOther = resizeSecond ? minFirstSize : minSecondSize;
            newSize = Math.max(minTarget, newSize);
            newSize = Math.min(containerSize - minOther, newSize);

            targetPanel.style[prop] = `${newSize}px`;
            targetPanel.style.flex = 'none';

            // Trigger resize events for canvas elements
            window.dispatchEvent(new Event('resize'));
        },
        onResizeEnd: () => {
            // Persist size
            if (storageKey) {
                const prop = direction === 'horizontal' ? 'height' : 'width';
                const size = targetPanel.getBoundingClientRect()[prop];
                localStorage.setItem(storageKey, String(Math.round(size)));
            }
        },
    });

    // Insert handle between panels
    handle.insertBetween(firstPanel, secondPanel);

    return handle;
}
