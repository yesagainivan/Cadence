/**
 * ADSR Envelope Editor Component
 * 
 * A Canvas-based visual editor for Attack, Decay, Sustain, Release parameters.
 * Displays the envelope curve and allows interactive dragging of control points.
 */

/** ADSR values as [attack, decay, sustain, release] in ms/percentage */
export type ADSRValues = [number, number, number, number];

/** Callback when ADSR values change */
export type ADSRChangeHandler = (values: ADSRValues) => void;

/** Default ADSR values */
export const DEFAULT_ADSR: ADSRValues = [10, 100, 70, 200];

/** Parameter ranges */
const RANGES = {
    attack: { min: 1, max: 500, unit: 'ms' },
    decay: { min: 1, max: 500, unit: 'ms' },
    sustain: { min: 0, max: 100, unit: '%' },
    release: { min: 1, max: 1000, unit: 'ms' },
};

/** Colors matching the editor theme */
const COLORS = {
    background: '#21242b',         // --color-bg-inset
    grid: 'rgba(255, 255, 255, 0.04)',
    curve: '#7fb069',              // earthy green (success)
    curveFill: 'rgba(127, 176, 105, 0.15)',
    controlPoint: '#7099aa',       // --color-accent
    controlPointHover: '#5a7d8d',  // --color-accent-hover
    text: '#6b6560',               // --color-fg-subtle
    label: '#7099aa',              // --color-accent
};

/**
 * Interactive ADSR Envelope Editor
 */
export class ADSREditor {
    private container: HTMLElement;
    private canvas: HTMLCanvasElement;
    private ctx: CanvasRenderingContext2D;
    private valuesDisplay: HTMLElement;

    private values: ADSRValues;
    private onChange: ADSRChangeHandler | null = null;

    // Dragging state
    private dragging: 'attack' | 'decay' | 'sustain' | 'release' | null = null;
    private hovered: 'attack' | 'decay' | 'sustain' | 'release' | null = null;

    // Canvas dimensions
    private width = 0;
    private height = 0;
    private padding = { top: 10, right: 10, bottom: 25, left: 10 };

    constructor(container: HTMLElement, initialValues?: ADSRValues) {
        this.container = container;
        this.values = initialValues ? [...initialValues] as ADSRValues : [...DEFAULT_ADSR];

        // Create canvas
        this.canvas = document.createElement('canvas');
        this.canvas.className = 'adsr-canvas';
        container.appendChild(this.canvas);

        const ctx = this.canvas.getContext('2d');
        if (!ctx) throw new Error('Could not get canvas 2D context');
        this.ctx = ctx;

        // Create values display
        this.valuesDisplay = document.createElement('div');
        this.valuesDisplay.className = 'adsr-values';
        container.appendChild(this.valuesDisplay);

        // Setup event listeners
        this.setupEventListeners();

        // Initial render
        this.resize();
        this.render();
        this.updateValuesDisplay();

        // Handle resize
        const resizeObserver = new ResizeObserver(() => this.resize());
        resizeObserver.observe(container);
    }

    /**
     * Set the change callback
     */
    onValueChange(handler: ADSRChangeHandler): void {
        this.onChange = handler;
    }

    /**
     * Update the ADSR values (from external source)
     */
    update(values: ADSRValues): void {
        this.values = [...values] as ADSRValues;
        this.render();
        this.updateValuesDisplay();
    }

    /**
     * Get current values
     */
    getValues(): ADSRValues {
        return [...this.values] as ADSRValues;
    }

    private resize(): void {
        const rect = this.container.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;

        this.width = rect.width;
        this.height = 80;

        this.canvas.width = this.width * dpr;
        this.canvas.height = this.height * dpr;
        this.canvas.style.width = `${this.width}px`;
        this.canvas.style.height = `${this.height}px`;

        this.ctx.scale(dpr, dpr);
        this.render();
    }

    private setupEventListeners(): void {
        this.canvas.addEventListener('mousedown', this.onMouseDown.bind(this));
        this.canvas.addEventListener('mousemove', this.onMouseMove.bind(this));
        this.canvas.addEventListener('mouseup', this.onMouseUp.bind(this));
        this.canvas.addEventListener('mouseleave', this.onMouseLeave.bind(this));
    }

    private onMouseDown(e: MouseEvent): void {
        const point = this.getPointAtPosition(e.offsetX, e.offsetY);
        if (point) {
            this.dragging = point;
            this.canvas.style.cursor = 'grabbing';
        }
    }

    private onMouseMove(e: MouseEvent): void {
        if (this.dragging) {
            this.updateValueFromDelta(e.movementX, e.movementY);
            this.render();
            this.updateValuesDisplay();
            if (this.onChange) {
                this.onChange(this.getValues());
            }
        } else {
            // Hover detection
            const point = this.getPointAtPosition(e.offsetX, e.offsetY);
            if (point !== this.hovered) {
                this.hovered = point;
                this.canvas.style.cursor = point ? 'grab' : 'default';
                this.render();
            }
        }
    }

    private onMouseUp(): void {
        this.dragging = null;
        this.canvas.style.cursor = this.hovered ? 'grab' : 'default';
    }

    private onMouseLeave(): void {
        this.dragging = null;
        this.hovered = null;
        this.canvas.style.cursor = 'default';
        this.render();
    }

    /**
     * Get the control point at the given position
     */
    private getPointAtPosition(x: number, y: number): 'attack' | 'decay' | 'sustain' | 'release' | null {
        const points = this.getControlPoints();
        const radius = 8;

        for (const [name, point] of Object.entries(points)) {
            const dx = x - point.x;
            const dy = y - point.y;
            if (dx * dx + dy * dy < radius * radius) {
                return name as 'attack' | 'decay' | 'sustain' | 'release';
            }
        }
        return null;
    }

    /**
     * Update value based on mouse movement (delta)
     */
    private updateValueFromDelta(dx: number, dy: number): void {
        const plotHeight = this.height - this.padding.top - this.padding.bottom;

        // Scale factor: how many ms per pixel of movement?
        // Let's say 100px = 500ms -> 5ms/px
        const TIME_SCALE = 5;

        switch (this.dragging) {
            case 'attack': {
                const current = this.values[0];
                const newValue = current + (dx * TIME_SCALE);
                this.values[0] = Math.max(RANGES.attack.min, Math.min(RANGES.attack.max, newValue));
                break;
            }
            case 'decay': {
                const current = this.values[1];
                const newValue = current + (dx * TIME_SCALE);
                this.values[1] = Math.max(RANGES.decay.min, Math.min(RANGES.decay.max, newValue));
                break;
            }
            case 'sustain': {
                const current = this.values[2];
                // dy is positive down, so negative dy means increase sustain (up)
                // 100% is top, 0% is bottom. 
                // plotHeight pixels = 100%.
                const percentChange = -(dy / plotHeight) * 100;
                const newValue = current + percentChange;
                this.values[2] = Math.max(RANGES.sustain.min, Math.min(RANGES.sustain.max, newValue));
                break;
            }
            case 'release': {
                const current = this.values[3];
                const newValue = current + (dx * TIME_SCALE);
                this.values[3] = Math.max(RANGES.release.min, Math.min(RANGES.release.max, newValue));
                break;
            }
        }
    }

    /**
     * Calculate control point positions
     */
    private getControlPoints(): { attack: { x: number; y: number }; decay: { x: number; y: number }; sustain: { x: number; y: number }; release: { x: number; y: number } } {
        const plotWidth = this.width - this.padding.left - this.padding.right;
        const plotHeight = this.height - this.padding.top - this.padding.bottom;

        const [a, d, s, r] = this.values;
        const totalTime = a + d + r + 100; // Add sustain hold time

        const attackX = this.padding.left + (a / totalTime) * plotWidth;
        const decayX = attackX + (d / totalTime) * plotWidth;
        const sustainEndX = decayX + (100 / totalTime) * plotWidth; // Sustain hold
        const releaseX = sustainEndX + (r / totalTime) * plotWidth;

        const peakY = this.padding.top;
        const sustainY = this.padding.top + plotHeight * (1 - s / 100);
        const bottomY = this.padding.top + plotHeight;

        return {
            attack: { x: attackX, y: peakY },
            decay: { x: decayX, y: sustainY },
            sustain: { x: sustainEndX, y: sustainY },
            release: { x: releaseX, y: bottomY },
        };
    }

    /**
     * Render the envelope visualization
     */
    private render(): void {
        const ctx = this.ctx;
        const plotHeight = this.height - this.padding.top - this.padding.bottom;

        // Clear
        ctx.fillStyle = COLORS.background;
        ctx.fillRect(0, 0, this.width, this.height);

        // Grid lines
        ctx.strokeStyle = COLORS.grid;
        ctx.lineWidth = 1;
        for (let i = 0; i <= 4; i++) {
            const y = this.padding.top + (plotHeight * i) / 4;
            ctx.beginPath();
            ctx.moveTo(this.padding.left, y);
            ctx.lineTo(this.width - this.padding.right, y);
            ctx.stroke();
        }

        // Get control points
        const points = this.getControlPoints();
        const startX = this.padding.left;
        const startY = this.padding.top + plotHeight;

        // Draw filled area
        ctx.fillStyle = COLORS.curveFill;
        ctx.beginPath();
        ctx.moveTo(startX, startY);
        ctx.lineTo(points.attack.x, points.attack.y);
        ctx.lineTo(points.decay.x, points.decay.y);
        ctx.lineTo(points.sustain.x, points.sustain.y);
        ctx.lineTo(points.release.x, points.release.y);
        ctx.lineTo(points.release.x, startY);
        ctx.closePath();
        ctx.fill();

        // Draw curve line
        ctx.strokeStyle = COLORS.curve;
        ctx.lineWidth = 2;
        ctx.lineJoin = 'round';
        ctx.beginPath();
        ctx.moveTo(startX, startY);
        ctx.lineTo(points.attack.x, points.attack.y);
        ctx.lineTo(points.decay.x, points.decay.y);
        ctx.lineTo(points.sustain.x, points.sustain.y);
        ctx.lineTo(points.release.x, points.release.y);
        ctx.stroke();

        // Draw control points
        const controlRadius = 5;
        for (const [name, point] of Object.entries(points)) {
            const isActive = this.dragging === name || this.hovered === name;
            ctx.fillStyle = isActive ? COLORS.controlPointHover : COLORS.controlPoint;
            ctx.beginPath();
            ctx.arc(point.x, point.y, isActive ? controlRadius + 2 : controlRadius, 0, Math.PI * 2);
            ctx.fill();
        }

        // Labels
        ctx.fillStyle = COLORS.text;
        ctx.font = '10px Inter, sans-serif';
        ctx.textAlign = 'center';

        const labels = ['A', 'D', 'S', 'R'];
        const pointsArr = [points.attack, points.decay, points.sustain, points.release];
        pointsArr.forEach((p, i) => {
            ctx.fillText(labels[i], p.x, this.height - 5);
        });
    }

    /**
     * Update the values display
     */
    private updateValuesDisplay(): void {
        const [a, d, s, r] = this.values;
        this.valuesDisplay.innerHTML = `
            <span class="adsr-value"><span class="adsr-label">A</span>${Math.round(a)}ms</span>
            <span class="adsr-value"><span class="adsr-label">D</span>${Math.round(d)}ms</span>
            <span class="adsr-value"><span class="adsr-label">S</span>${Math.round(s)}%</span>
            <span class="adsr-value"><span class="adsr-label">R</span>${Math.round(r)}ms</span>
        `;
    }
}

