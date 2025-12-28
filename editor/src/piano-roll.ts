/**
 * Piano Roll Visualization Component
 * 
 * Canvas-based piano roll that displays patterns from Cadence code.
 * Uses rich event data (NoteInfo with MIDI notes, names, etc.) for accurate rendering.
 */

import type { PlayEvent, PatternEvents } from './cadence-wasm';

// Note colors by pitch class - muted, earthy palette
const PITCH_COLORS: Record<number, string> = {
    0: '#c9736f',   // C  - muted red
    1: '#b86662',   // C# - dark red
    2: '#d4a656',   // D  - warm gold
    3: '#c49a4e',   // D# - dark gold
    4: '#d9bf6a',   // E  - light gold
    5: '#7fb069',   // F  - earthy green
    6: '#6e9d5c',   // F# - dark green
    7: '#7099aa',   // G  - blue-grey (accent)
    8: '#5d8495',   // G# - dark blue-grey
    9: '#9a8fbd',   // A  - muted purple
    10: '#877baa',  // A# - dark purple
    11: '#bf8fa3',  // B  - dusty rose
};

// Piano key properties
const KEY_LABEL_WIDTH = 40;
const HEADER_HEIGHT = 24;

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
        this.events = patternEvents.events;
        this.beatsPerCycle = patternEvents.beats_per_cycle;
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
        ctx.fillStyle = '#21242b';  // --color-bg-inset
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

        ctx.strokeStyle = 'rgba(255, 255, 255, 0.05)';
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
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.1)';
            } else {
                ctx.strokeStyle = 'rgba(255, 255, 255, 0.03)';
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

        for (let midi = this.minNote; midi < this.maxNote; midi++) {
            const pitchClass = midi % 12;
            const octave = Math.floor(midi / 12) - 1;
            const y = HEADER_HEIGHT + (this.maxNote - midi - 0.5) * noteHeight;

            // Only label C notes and every few others
            if (pitchClass === 0) {
                ctx.fillStyle = '#e0dcd4';  // --color-fg
                ctx.fillText(`C${octave}`, KEY_LABEL_WIDTH - 6, y);
            } else if (pitchClass === 4 || pitchClass === 7) {
                ctx.fillStyle = '#6b6560';  // --color-fg-subtle
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
        ctx.fillStyle = '#282c34';  // --color-bg
        ctx.fillRect(KEY_LABEL_WIDTH, 0, width - KEY_LABEL_WIDTH, HEADER_HEIGHT);

        ctx.font = '11px Inter, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = '#6b6560';  // --color-fg-subtle

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

            const x = KEY_LABEL_WIDTH + event.start_beat * beatWidth;
            const w = event.duration * beatWidth - 2;  // Small gap between notes

            for (const note of event.notes) {
                const y = HEADER_HEIGHT + (this.maxNote - note.midi - 1) * noteHeight;
                const h = noteHeight - 2;

                // Note color based on pitch class
                const color = PITCH_COLORS[note.pitch_class] || '#888';

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
        ctx.strokeStyle = '#7099aa';  // --color-accent
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(x, HEADER_HEIGHT);
        ctx.lineTo(x, height);
        ctx.stroke();

        // Draw playhead triangle at top
        ctx.fillStyle = '#7099aa';  // --color-accent
        ctx.beginPath();
        ctx.moveTo(x, HEADER_HEIGHT);
        ctx.lineTo(x - 6, HEADER_HEIGHT - 8);
        ctx.lineTo(x + 6, HEADER_HEIGHT - 8);
        ctx.closePath();
        ctx.fill();
    }
}
