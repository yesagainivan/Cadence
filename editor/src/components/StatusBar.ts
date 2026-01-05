/**
 * StatusBar Component
 * 
 * Footer status bar showing validation status and cursor position.
 */

export type StatusType = 'success' | 'error' | 'warning' | 'info';

/**
 * StatusBar manages the footer status display
 */
export class StatusBar {
    private statusEl: HTMLElement | null;
    private cursorPosEl: HTMLElement | null;

    constructor() {
        this.statusEl = document.getElementById('status');
        this.cursorPosEl = document.getElementById('cursor-pos');
    }

    /**
     * Set status message with type indicator
     */
    setStatus(message: string, type: StatusType = 'info'): void {
        if (!this.statusEl) return;

        const colors: Record<StatusType, string> = {
            success: 'var(--color-success)',
            error: 'var(--color-error)',
            warning: 'var(--color-warning)',
            info: 'var(--color-info)',
        };

        this.statusEl.innerHTML = `<span class="status-dot"></span>${message}`;
        const dot = this.statusEl.querySelector('.status-dot') as HTMLElement;
        if (dot) {
            dot.style.backgroundColor = colors[type];
        }
    }

    /**
     * Set cursor position display
     */
    setCursor(line: number, col: number): void {
        if (this.cursorPosEl) {
            this.cursorPosEl.textContent = `Ln ${line}, Col ${col}`;
        }
    }

    /**
     * Shortcut: set valid status
     */
    setValid(): void {
        this.setStatus('Valid', 'success');
    }

    /**
     * Shortcut: set error status
     */
    setError(message: string): void {
        this.setStatus(message, 'error');
    }
}
