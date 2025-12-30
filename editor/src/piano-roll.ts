/**
 * Piano Roll Visualization Component
 * 
 * Canvas-based piano roll that displays patterns from Cadence code.
 * Uses rich event data (NoteInfo with MIDI notes, names, etc.) for accurate rendering.
 */

import type { PlayEvent, PatternEvents } from './cadence-wasm';
import { rationalToFloat } from './cadence-wasm';
import { getTheme, onThemeChange } from './theme';

// Piano key properties
const KEY_LABEL_WIDTH = 40;
const HEADER_HEIGHT = 24;

/** Lock target: what the piano roll is locked on */
type LockTarget =
    | { type: 'variable'; name: string }
    | { type: 'position'; position: number };

/**
 * Piano Roll visualization component
 */
export class PianoRoll {
    private canvas: HTMLCanvasElement;
    private ctx: CanvasRenderingContext2D;
    private events: PlayEvent[] = [];

    // Visible range (MIDI note numbers)
    private minNote: number = 48;  // C3
    private maxNote: number = 84;  // C6
    private totalBeats: number = 4;
    private beatsPerCycle: number = 4;  // Explicit cycle length from pattern

    // Playhead state
    private playheadBeat: number = 0;
    private animationFrameId: number | null = null;

    // Lock state
    private locked: boolean = false;
    private lockTarget: LockTarget | null = null;
    private getEventsCallback: ((code: string, position: number) => PatternEvents | null) | null = null;

    constructor(canvasId: string) {
        const canvas = document.getElementById(canvasId) as HTMLCanvasElement;
        if (!canvas) {
            throw new Error(`Canvas element '${canvasId}' not found`);
        }
        this.canvas = canvas;

        const ctx = canvas.getContext('2d');
        if (!ctx) {
            throw new Error('Failed to get 2D context');
        }
        this.ctx = ctx;

        // Handle resize
        this.resize();
        window.addEventListener('resize', () => this.resize());

        // Re-render on theme change
        onThemeChange(() => this.render(this.events));

        // Set up lock button
        this.setupLockButton();
    }

    /**
     * Set the callback for fetching events at a position
     * This allows the piano roll to refresh locked content on code changes
     */
    public setEventsCallback(callback: (code: string, position: number) => PatternEvents | null): void {
        this.getEventsCallback = callback;
    }

    /**
     * Set up the lock button event listener
     */
    private setupLockButton(): void {
        const lockBtn = document.getElementById('piano-roll-lock');
        if (lockBtn) {
            lockBtn.addEventListener('click', () => this.toggleLock());
        }
    }

    /**
     * Lock on the current pattern with the given context
     */
    public lockOn(context: { variable_name: string | null; span: { utf16_start: number } } | null): void {
        if (!context || this.events.length === 0) return;

        // Determine what to lock on
        if (context.variable_name) {
            // Lock on variable name
            this.lockTarget = { type: 'variable', name: context.variable_name };
        } else {
            // Lock on position (for anonymous patterns like `play "C E D"`)
            this.lockTarget = { type: 'position', position: context.span.utf16_start };
        }

        this.locked = true;
        this.updateLockUI(true);
    }

    /**
     * Toggle lock state - needs context to know what to lock on
     */
    public toggleLock(context?: { variable_name: string | null; span: { utf16_start: number } } | null): void {
        if (!this.locked && this.events.length > 0) {
            // Lock: need context to know what to lock on
            if (context) {
                this.lockOn(context);
            } else {
                // Fallback: lock on current position (will be set by updateWithContext)
                this.locked = true;
                this.updateLockUI(true);
            }
        } else {
            // Unlock
            this.locked = false;
            this.lockTarget = null;
            this.updateLockUI(false);
        }
    }

    /**
     * Update lock button UI
     */
    private updateLockUI(locked: boolean): void {
        const lockBtn = document.getElementById('piano-roll-lock');
        const unlockedIcon = lockBtn?.querySelector('.icon-unlocked') as HTMLElement;
        const lockedIcon = lockBtn?.querySelector('.icon-locked') as HTMLElement;

        if (locked) {
            if (unlockedIcon) unlockedIcon.style.display = 'none';
            if (lockedIcon) lockedIcon.style.display = 'block';
            if (lockBtn) lockBtn.classList.add('active');
        } else {
            if (unlockedIcon) unlockedIcon.style.display = 'block';
            if (lockedIcon) lockedIcon.style.display = 'none';
            if (lockBtn) lockBtn.classList.remove('active');
        }
    }

    /**
     * Refresh the locked pattern with new code
     * Called when code changes to re-evaluate the locked statement
     */
    public refreshLocked(code: string, findVariablePosition?: (code: string, varName: string) => number | null): void {
        if (!this.locked || !this.lockTarget || !this.getEventsCallback) return;

        let position: number | null = null;

        if (this.lockTarget.type === 'variable' && findVariablePosition) {
            // Find the variable's current position in the code
            position = findVariablePosition(code, this.lockTarget.name);
        } else if (this.lockTarget.type === 'position') {
            // Use the stored position (may drift if code before it changes)
            position = this.lockTarget.position;
        }

        if (position !== null) {
            const events = this.getEventsCallback(code, position);
            if (events && events.events.length > 0) {
                this.events = events.events;
                this.beatsPerCycle = rationalToFloat(events.beats_per_cycle);
                this.calculateRange();
                this.render(this.events);
            }
        }
    }

    /**
     * Check if piano roll is locked
     */
    public isLocked(): boolean {
        return this.locked;
    }

    /**
     * Get what we're locked on (for debugging/display)
     */
    public getLockTarget(): LockTarget | null {
        return this.lockTarget;
    }

    /**
     * Resize canvas to fill container
     */
    private resize(): void {
        const container = this.canvas.parentElement;
        if (!container) return;

        const rect = container.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;

        this.canvas.width = rect.width * dpr;
        this.canvas.height = rect.height * dpr;
        this.canvas.style.width = `${rect.width}px`;
        this.canvas.style.height = `${rect.height}px`;

        this.ctx.scale(dpr, dpr);
        this.render(this.events);
    }

    /**
     * Update and render new events
     * @param patternEvents - Pattern events with cycle timing info
     */
    update(patternEvents: PatternEvents): void {
        // If locked, ignore updates
        if (this.locked) return;

        this.events = patternEvents.events;
        this.beatsPerCycle = rationalToFloat(patternEvents.beats_per_cycle);
        this.calculateRange();
        this.render(this.events);
    }

    /**
     * Calculate visible note range from events
     */
    private calculateRange(): void {
        // Use explicit cycle length from pattern
        this.totalBeats = Math.max(1, Math.ceil(this.beatsPerCycle));

        if (this.events.length === 0) {
            this.minNote = 48;
            this.maxNote = 84;
            return;
        }

        let minMidi = 127;
        let maxMidi = 0;

        for (const event of this.events) {
            for (const note of event.notes) {
                minMidi = Math.min(minMidi, note.midi);
                maxMidi = Math.max(maxMidi, note.midi);
            }
        }

        // Add padding
        this.minNote = Math.max(0, minMidi - 4);
        this.maxNote = Math.min(127, maxMidi + 4);
    }

    /**
     * Main render method
     */
    private render(events: PlayEvent[]): void {
        const { ctx, canvas } = this;
        const width = canvas.width / (window.devicePixelRatio || 1);
        const height = canvas.height / (window.devicePixelRatio || 1);

        // Clear with theme background
        const colors = getTheme().colors;
        ctx.fillStyle = colors.bgInset;
        ctx.fillRect(0, 0, width, height);

        // Draw components
        this.drawBeatGrid(width, height);
        this.drawPianoKeys(height);
        this.drawNotes(events);
        this.drawBeatHeader(width);
        this.drawPlayhead(width, height);
    }

    /**
     * Draw beat grid lines
     */
    private drawBeatGrid(width: number, height: number): void {
        const { ctx } = this;
        const gridWidth = width - KEY_LABEL_WIDTH;
        const beatWidth = gridWidth / this.totalBeats;

        const colors = getTheme().colors;
        ctx.strokeStyle = colors.border;
        ctx.lineWidth = 1;

        // Vertical beat lines
        for (let beat = 0; beat <= this.totalBeats; beat++) {
            const x = KEY_LABEL_WIDTH + beat * beatWidth;
            ctx.beginPath();
            ctx.moveTo(x, HEADER_HEIGHT);
            ctx.lineTo(x, height);
            ctx.stroke();
        }

        // Horizontal note lines (every note)
        const noteRange = this.maxNote - this.minNote;
        const noteHeight = (height - HEADER_HEIGHT) / noteRange;

        for (let midi = this.minNote; midi <= this.maxNote; midi++) {
            const y = HEADER_HEIGHT + (this.maxNote - midi) * noteHeight;

            // Highlight C notes
            if (midi % 12 === 0) {
                ctx.strokeStyle = colors.borderSubtle;
            } else {
                ctx.strokeStyle = colors.bgHover;
            }

            ctx.beginPath();
            ctx.moveTo(KEY_LABEL_WIDTH, y);
            ctx.lineTo(width, y);
            ctx.stroke();
        }
    }

    /**
     * Draw piano key labels on the left
     */
    private drawPianoKeys(height: number): void {
        const { ctx } = this;
        const noteRange = this.maxNote - this.minNote;
        const noteHeight = (height - HEADER_HEIGHT) / noteRange;

        const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

        ctx.font = '10px Inter, sans-serif';
        ctx.textAlign = 'right';
        ctx.textBaseline = 'middle';

        const colors = getTheme().colors;
        for (let midi = this.minNote; midi < this.maxNote; midi++) {
            const pitchClass = midi % 12;
            const octave = Math.floor(midi / 12) - 1;
            const y = HEADER_HEIGHT + (this.maxNote - midi - 0.5) * noteHeight;

            // Only label C notes and every few others
            if (pitchClass === 0) {
                ctx.fillStyle = colors.fg;
                ctx.fillText(`C${octave}`, KEY_LABEL_WIDTH - 6, y);
            } else if (pitchClass === 4 || pitchClass === 7) {
                ctx.fillStyle = colors.fgSubtle;
                ctx.fillText(NOTE_NAMES[pitchClass], KEY_LABEL_WIDTH - 6, y);
            }
        }
    }

    /**
     * Draw beat numbers at the top
     */
    private drawBeatHeader(width: number): void {
        const { ctx } = this;
        const gridWidth = width - KEY_LABEL_WIDTH;
        const beatWidth = gridWidth / this.totalBeats;

        // Header background
        const colors = getTheme().colors;
        ctx.fillStyle = colors.bg;
        ctx.fillRect(KEY_LABEL_WIDTH, 0, width - KEY_LABEL_WIDTH, HEADER_HEIGHT);

        ctx.font = '11px Inter, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = colors.fgSubtle;

        for (let beat = 0; beat < this.totalBeats; beat++) {
            const x = KEY_LABEL_WIDTH + (beat + 0.5) * beatWidth;
            ctx.fillText(`${beat + 1}`, x, HEADER_HEIGHT / 2);
        }
    }

    /**
     * Draw note rectangles
     */
    private drawNotes(events: PlayEvent[]): void {
        const { ctx, canvas } = this;
        const width = canvas.width / (window.devicePixelRatio || 1);
        const height = canvas.height / (window.devicePixelRatio || 1);

        const gridWidth = width - KEY_LABEL_WIDTH;
        const beatWidth = gridWidth / this.totalBeats;
        const noteRange = this.maxNote - this.minNote;
        const noteHeight = (height - HEADER_HEIGHT) / noteRange;

        for (const event of events) {
            if (event.is_rest) continue;

            const startBeat = rationalToFloat(event.start_beat);
            const durationBeats = rationalToFloat(event.duration);
            const x = KEY_LABEL_WIDTH + startBeat * beatWidth;
            const w = durationBeats * beatWidth - 2;  // Small gap between notes

            for (const note of event.notes) {
                const y = HEADER_HEIGHT + (this.maxNote - note.midi - 1) * noteHeight;
                const h = noteHeight - 2;

                // Note color based on pitch class from theme
                const noteColors = getTheme().colors.noteColors;
                const color = noteColors[note.pitch_class] || getTheme().colors.fgMuted;

                // Draw note rectangle with rounded corners
                ctx.fillStyle = color;
                this.roundRect(x + 1, y + 1, Math.max(w - 2, 4), Math.max(h, 4), 3);

                // Draw note name for longer notes
                if (w > 30) {
                    ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
                    ctx.font = 'bold 9px Inter, sans-serif';
                    ctx.textAlign = 'left';
                    ctx.textBaseline = 'middle';
                    ctx.fillText(note.name, x + 5, y + h / 2 + 1);
                }
            }
        }
    }

    /**
     * Draw a rounded rectangle
     */
    private roundRect(x: number, y: number, w: number, h: number, r: number): void {
        const { ctx } = this;
        ctx.beginPath();
        ctx.moveTo(x + r, y);
        ctx.lineTo(x + w - r, y);
        ctx.quadraticCurveTo(x + w, y, x + w, y + r);
        ctx.lineTo(x + w, y + h - r);
        ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
        ctx.lineTo(x + r, y + h);
        ctx.quadraticCurveTo(x, y + h, x, y + h - r);
        ctx.lineTo(x, y + r);
        ctx.quadraticCurveTo(x, y, x + r, y);
        ctx.closePath();
        ctx.fill();
    }

    /**
     * Clear the piano roll
     */
    clear(): void {
        this.events = [];
        this.render([]);
    }

    /**
     * Set the playhead position (in beats)
     */
    setPlayhead(beat: number): void {
        this.playheadBeat = beat;
        this.render(this.events);
    }

    /**
     * Start animating the playhead
     */
    startAnimation(getPosition: () => number | null): void {
        const animate = () => {
            const beat = getPosition();
            if (beat !== null) {
                this.playheadBeat = beat;
                this.render(this.events);
                this.animationFrameId = requestAnimationFrame(animate);
            }
        };
        this.animationFrameId = requestAnimationFrame(animate);
    }

    /**
     * Stop animating the playhead
     */
    stopAnimation(): void {
        if (this.animationFrameId !== null) {
            cancelAnimationFrame(this.animationFrameId);
            this.animationFrameId = null;
        }
        this.playheadBeat = 0;
        this.render(this.events);
    }

    /**
     * Draw the playhead line
     */
    private drawPlayhead(width: number, height: number): void {
        if (this.playheadBeat <= 0) return;

        const { ctx } = this;
        const gridWidth = width - KEY_LABEL_WIDTH;
        const beatWidth = gridWidth / this.totalBeats;

        // Wrap playhead within visible beats
        const wrappedBeat = this.playheadBeat % this.totalBeats;
        const x = KEY_LABEL_WIDTH + wrappedBeat * beatWidth;

        // Draw playhead line
        const colors = getTheme().colors;
        ctx.strokeStyle = colors.accent;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(x, HEADER_HEIGHT);
        ctx.lineTo(x, height);
        ctx.stroke();

        // Draw playhead triangle at top
        ctx.fillStyle = colors.accent;
        ctx.beginPath();
        ctx.moveTo(x, HEADER_HEIGHT);
        ctx.lineTo(x - 6, HEADER_HEIGHT - 8);
        ctx.lineTo(x + 6, HEADER_HEIGHT - 8);
        ctx.closePath();
        ctx.fill();
    }
}
