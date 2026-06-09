---
name: xtermjs-parser-hooks
description: Write, debug, or review xterm.js parser hooks for terminal sequences (ESC, CSI, DCS, OSC, APC), including custom sequences, parameter handling, async hooks, and their lifecycle limitations. Use when working with term.parser.register*Handler or interpreting/extending terminal escape sequences in xterm.js.
---

# xterm.js Parser Hooks & Terminal Sequences

Use this skill when writing, debugging, or reviewing xterm.js parser hooks: intercepting built-in terminal sequences or defining custom ones via `term.parser.register*Handler`. In this project the terminal lives in the frontend (`src/main.ts`, xterm.js); register hooks on the `Terminal` instance there.

## Choosing the right hook

xterm.js exposes hooks for these sequence types. Match your sequence to the correct registrar:

| Registrar | Type | Format | Use for |
|-----------|------|--------|---------|
| `parser.registerEscHandler` | ESC | `ESC <intermediates> <final>` | Simple control functions, no numeric params |
| `parser.registerCsiHandler` | CSI | `CSI <prefix> P1;P2;... <intermediates> <final>` | Control functions with positive integer params |
| `parser.registerDcsHandler` | DCS | `DCS <prefix> P1;P2;... <intermediates> <final> <payload> ST` | Numeric params **and** ASCII string payload (e.g. BASE64 binary) |
| `parser.registerOscHandler` | OSC | `OSC <identifier> ; <payload> ST` | String payload keyed by a numeric identifier |
| `parser.registerApcHandler` | APC | `APC <intermediates> <final> <payload> ST` | Application program command payload |

Byte-range constraints for identifiers (`IFunctionIdentifier`), validated and thrown on by xterm.js:
- prefix: one byte `\x3C..\x3F` (CSI/DCS only)
- intermediates: up to 2 bytes `\x20..\x2F`
- CSI/DCS final: one byte `\x40..\x7E`; ESC/APC final: one byte `\x30..\x7E`

Not hookable: single-byte C0/C1 control functions and `PRINT`. `PM`/`SOS` are recognized but unsupported.

## Lifecycle rules (critical)

xterm.js processes input in time slices: synchronous `term.write(data, cb)` buffering → asynchronous input processing (parser + hooks, in-band) → screen updates → event processing. Consequences for hooks:

- **Keep hooks synchronous and fast.** They block input processing; the terminal state will not advance until the hook returns.
- **Return a boolean.** `false` = sequence not fully handled, keep probing other handlers (almost always correct for built-in sequences). `true` = stop further processing.
- **Never assume a chunk reaches the screen.** Under heavy input, intermediate states may never render.

## Execution order

Handlers register like event handlers but are probed in **reverse registration order** (most recently registered first). The parser stops at the first handler that returns `true`.

```ts
const h = term.parser.registerCsiHandler({ final: 'H' }, params => {
  // ...
  return false; // also probe earlier/default handlers
});
// later:
h.dispose();
```

For built-in sequences, do **not** return `true` (which skips the default handler) unless you fully intend to replace built-in behavior.

## Numeric parameter handling

The parser treats CSI/DCS numeric params as optional and applies zero-default mode (ZDM). Examples for a `CUP` (`CSI Ps ; Ps H`):

- `CSI H` → `[0]`
- `CSI 10 H` → `[10]`
- `CSI ; H` → `[0, 0]`
- `CSI 10 ; 20 H` → `[10, 20]`
- `CSI 1;2;3;4;5 H` → `[1, 2, 3, 4, 5]`

The parser does **not** know per-sequence default/clamp rules — apply them yourself. Typical `CUP` normalization (0 → 1, fill missing, ignore excess):

```ts
const h = term.parser.registerCsiHandler({ final: 'H' }, params => {
  let p = params.toArray().map(v => v || 1); // 0 defaults to 1
  while (p.length < 2) p.push(1);            // fill missing
  p = p.slice(0, 2);                         // ignore excess
  console.log({ row: p[0], col: p[1] });
  return false;
});
```

(`params` is an `IParams`; use `.toArray()` / index access. Always supply sane defaults since any param may be missing.)

## Custom sequences

Only introduce custom sequences as a last resort — they break compatibility with other terminal emulators.

1. **Pick a carrier type.** CSI carries only positive integers (limited to 32 params, each up to 2³¹−1). OSC/DCS carry ASCII string payload (good for BASE64 binary); DCS additionally supports numeric params like CSI.
2. **Pick a free function identifier.** Consult ECMA-48 / DEC STD 070 to avoid clashes. Rule of thumb: use private-area prefixes for CSI, or a high number (>1000) for OSC.
3. **Treat all OSC/DCS payload as untrusted** and sanitize before use.

Example — a DCS hook `DCS ? Ps a Pt ST`:

```ts
const handler = term.parser.registerDcsHandler({ prefix: '?', final: 'a' }, (params, data) => {
  const pitch = params.params[0] || 440; // sane default
  const decoded = convertB64(data);
  if (isValid(decoded)) doSomething(pitch, decoded);
  return true;
});
// emitted as: \x1bP?<pitch>a<base64>\x1b\\
```

## Async hooks (use sparingly)

A handler may return `Promise<boolean>`. The parser unwinds the stack and pauses input processing until the promise settles, preserving sequence ordering.

```ts
term.parser.registerCsiHandler({ final: 'x' }, async params => {
  await someAsyncWork();
  return false;
});
```

Prefer a sync fast-path and only branch to async when unavoidable. Caveats:
- Significant throughput penalty (stack unwinding on every async hook).
- A promise that never settles renders the terminal unusable (partially recoverable via `Terminal.reset()`). xterm.js warns after 5s at `logLevel: 'warn'` or higher.
- Terminal state can still change between `await` points (resize, `reset()`). Re-check assumptions after each await.
- Collect state and apply it in one late mutation to avoid leaving garbage on async error.

Good reasons to go async: depending on async browser APIs, heavy computation that would block the main thread, flow-control/backpressure, or rare sequences with negligible impact on the input stream.

## Limitations to keep in mind

- **SGR-style parameter stacking** cannot be filtered per-parameter when sent as one call (`SGR 0;1;2`).
- **Reduced terminal state** is exposed via the public API; some internals are unavailable.
- **OSC/DCS payloads are capped at 10MB.**

## References

For sequence semantics and default behaviors, consult ECMA-48, DEC STD 070, the VT520 Programmer Information (vt100.net), and the xterm.js "supported sequences" list before implementing a hook.
