---

## Editor: Emoji Position Mismatch ✅ Fixed

**Status**: Resolved via UTF-16 offset tracking

**Problem**: When code contains emojis (e.g., `// Welcome 🎵`), span positions from Rust were off by 1+ characters in JavaScript.

**Root cause**:
- Rust `Vec<char>` counts emoji as 1 character
- JavaScript strings count emoji as 2 UTF-16 code units (surrogate pair)

**Fix Applied**:
- Added `utf16_offset` and `utf16_len` fields to Rust `Span` struct
- Lexer now tracks UTF-16 position alongside char position
- WASM bindings expose UTF-16 offsets in `HighlightSpan` and `SpanInfoJS`
- TypeScript editor uses UTF-16 positions for highlighting and property edits

**Impact**: Property edits now insert at correct positions even with emoji in code.

___
The challenge is that the Evaluator is calling the Interpreter, but currently Evaluator doesn't have a reference to an Interpreter. I need to think about this differently.

Looking at the code, the problem is that the Evaluator doesn't own an Interpreter. The Interpreter owns the Evaluator. So we can't call Interpreter methods from the Evaluator directly.

Alternative approach: Instead of calling Interpreter from Evaluator, I should enhance the Evaluator itself to properly handle statement execution using the local environment. Let me refactor the evaluator's function execution: