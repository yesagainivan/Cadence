/**
 * Properties Panel Component
 * 
 * Displays contextual property editors based on cursor position.
 * Integrates with get_context_at_cursor() API to show relevant controls.
 */

import type { CursorContext, EditableProperties } from './cadence-wasm';

/** Waveform options for the picker */
const WAVEFORMS = ['sine', 'saw', 'square', 'triangle'] as const;

/** Callback for when a property is changed */
export type PropertyChangeHandler = (change: PropertyChange) => void;

/** Describes a property change to apply to source code */
export interface PropertyChange {
    /** Type of change */
    type: 'waveform' | 'tempo' | 'volume' | 'envelope';
    /** New value */
    value: string | number | number[];
    /** UTF-16 start position of statement (for JS string operations) */
    spanStart: number;
    /** UTF-16 end position of statement */
    spanEnd: number;
}


/**
 * Properties Panel manages the sidebar panel that shows
 * editable properties for the current cursor context.
 */
export class PropertiesPanel {
    private container: HTMLElement;
    private currentContext: CursorContext | null = null;
    private onPropertyChange: PropertyChangeHandler | null = null;

    constructor(containerId: string) {
        const el = document.getElementById(containerId);
        if (!el) {
            throw new Error(`Properties panel container #${containerId} not found`);
        }
        this.container = el;
        this.renderEmpty();
    }

    /**
     * Set the callback for property changes
     */
    setOnPropertyChange(handler: PropertyChangeHandler): void {
        this.onPropertyChange = handler;
    }

    /**
     * Update the panel with new cursor context
     */
    update(context: CursorContext | null): void {
        this.currentContext = context;

        if (!context) {
            this.renderEmpty();
            return;
        }

        // Clear and rebuild panel content
        this.container.innerHTML = '';

        // Header with statement type
        const header = document.createElement('h3');
        header.textContent = 'Properties';
        this.container.appendChild(header);

        // Context info
        const contextInfo = document.createElement('div');
        contextInfo.className = 'context-info';
        contextInfo.innerHTML = `
            <span class="context-type">${this.formatType(context.statement_type)}</span>
            ${context.variable_name ? `<span class="context-var">${context.variable_name}</span>` : ''}
        `;
        this.container.appendChild(contextInfo);

        // Render properties based on context
        if (context.properties) {
            this.renderProperties(context.properties, context.value_type);
        } else if (context.value_type) {
            this.renderValueInfo(context.value_type);
        } else {
            this.renderNoProperties();
        }
    }

    /**
     * Render the empty/placeholder state
     */
    private renderEmpty(): void {
        this.container.innerHTML = `
            <h3>Properties</h3>
            <p class="placeholder">Move cursor to a statement to see properties</p>
        `;
    }

    /**
     * Render editable properties
     */
    private renderProperties(props: EditableProperties, valueType: string | null): void {
        const propsContainer = document.createElement('div');
        propsContainer.className = 'properties-list';

        // Waveform picker (for patterns)
        if (valueType === 'pattern' || props.waveform !== undefined) {
            propsContainer.appendChild(this.createWaveformPicker(props.waveform));
        }

        // Tempo (for tempo statements)
        if (props.tempo !== null && props.tempo !== undefined) {
            propsContainer.appendChild(this.createValueRow('Tempo', `${props.tempo} BPM`));
        }

        // Volume (for volume statements)
        if (props.volume !== null && props.volume !== undefined) {
            propsContainer.appendChild(this.createValueRow('Volume', `${Math.round(props.volume * 100)}%`));
        }

        // Beats per cycle (for patterns)
        if (props.beats_per_cycle !== null && props.beats_per_cycle !== undefined) {
            propsContainer.appendChild(this.createValueRow('Cycle', `${props.beats_per_cycle} beats`));
        }

        // Envelope (for patterns with custom envelope)
        if (props.envelope) {
            const [a, d, s, r] = props.envelope;
            propsContainer.appendChild(this.createValueRow('Envelope', `A:${a} D:${d} S:${s} R:${r}`));
        }

        this.container.appendChild(propsContainer);
    }

    /**
     * Create waveform picker element
     */
    private createWaveformPicker(currentWaveform: string | null | undefined): HTMLElement {
        const row = document.createElement('div');
        row.className = 'property-row';

        const label = document.createElement('label');
        label.textContent = 'Waveform';
        row.appendChild(label);

        const picker = document.createElement('div');
        picker.className = 'waveform-picker';

        for (const wf of WAVEFORMS) {
            const btn = document.createElement('button');
            btn.className = 'waveform-btn';
            btn.textContent = wf.charAt(0).toUpperCase() + wf.slice(1);
            btn.dataset.waveform = wf;

            // Mark active waveform
            const isActive = currentWaveform?.toLowerCase() === wf ||
                (currentWaveform === null && wf === 'sine');
            if (isActive) {
                btn.classList.add('active');
            }

            // Fire callback when waveform is selected
            btn.addEventListener('click', () => {
                if (this.onPropertyChange && this.currentContext) {
                    this.onPropertyChange({
                        type: 'waveform',
                        value: wf,
                        // Use UTF-16 positions for correct emoji/multi-byte character handling
                        spanStart: this.currentContext.span.utf16_start,
                        spanEnd: this.currentContext.span.utf16_end,
                    });
                }
            });


            picker.appendChild(btn);
        }

        row.appendChild(picker);
        return row;
    }

    /**
     * Create a read-only value row
     */
    private createValueRow(label: string, value: string): HTMLElement {
        const row = document.createElement('div');
        row.className = 'property-row';
        row.innerHTML = `
            <label>${label}</label>
            <span class="property-value">${value}</span>
        `;
        return row;
    }

    /**
     * Render info for values without editable properties
     */
    private renderValueInfo(valueType: string): void {
        const info = document.createElement('div');
        info.className = 'value-info';
        info.innerHTML = `
            <span class="value-type-badge">${valueType}</span>
        `;
        this.container.appendChild(info);
    }

    /**
     * Render message when no properties are available
     */
    private renderNoProperties(): void {
        const msg = document.createElement('p');
        msg.className = 'no-properties';
        msg.textContent = 'No editable properties';
        this.container.appendChild(msg);
    }

    /**
     * Format statement type for display
     */
    private formatType(type: string): string {
        return type.charAt(0).toUpperCase() + type.slice(1);
    }
}
