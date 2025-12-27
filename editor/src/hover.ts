import { hoverTooltip, type HoverTooltipSource } from "@codemirror/view";
import { getDocumentation, type DocItem } from "./cadence-wasm";

let cachedDocs: Map<string, DocItem> | null = null;

function ensureDocs() {
    if (cachedDocs) return;
    const docs = getDocumentation();
    if (docs && docs.length > 0) {
        cachedDocs = new Map();
        for (const doc of docs) {
            cachedDocs.set(doc.name, doc);
        }
    }
}

export const cadenceHover = hoverTooltip((view, pos, side) => {
    ensureDocs();
    if (!cachedDocs) {
        return null;
    }

    const { from, to, text } = view.state.doc.lineAt(pos);

    // Find word boundaries
    let start = pos;
    let end = pos;

    // Scan left
    while (start > from) {
        const char = text[start - from - 1];
        if (!/[\w\d_]/.test(char)) break;
        start--;
    }

    // Scan right
    while (end < to) {
        const char = text[end - from];
        if (!/[\w\d_]/.test(char)) break;
        end++;
    }

    if (start === end) return null;

    const word = text.slice(start - from, end - from);
    const doc = cachedDocs.get(word);

    if (!doc) return null;

    return {
        pos: start,
        end,
        above: true,
        create(view) {
            const dom = document.createElement("div");
            dom.className = "cm-tooltip-cursor";
            dom.style.padding = "4px 8px";
            dom.style.fontFamily = "monospace";
            dom.style.maxWidth = "400px";

            dom.innerHTML = `
                <div style="font-weight: bold; border-bottom: 1px solid #444; margin-bottom: 4px; padding-bottom: 2px;">
                    <span style="color: #61afef">${doc.name}</span>
                    <span style="float: right; color: #abb2bf; font-size: 0.8em; font-weight: normal">${doc.category}</span>
                </div>
                <div style="color: #98c379; margin-bottom: 4px;">${doc.signature}</div>
                <div style="color: #abb2bf; white-space: pre-wrap;">${doc.description}</div>
            `;
            return { dom };
        }
    };
});
