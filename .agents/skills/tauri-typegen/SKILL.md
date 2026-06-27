---
name: tauri-typegen
description: Guidance for regenerating and cleaning up Tauri type bindings via `pnpm tauri-typegen`. Use when running tauri-typegen, or editing files in `src/generated/` to remove unused imports/symbols left by the generator.
---

# Tauri Type Bindings

Tauri's type generator (`pnpm tauri-typegen`) regenerates the bindings in `src/generated/`. Per project convention in `AGENTS.md`, these files are not hand-edited for content — regenerate them instead. However, the generator leaves unused symbols in imports, which trips TypeScript and lint rules.

## Workflow

1. Make the desired backend changes in `src-tauri/` (commands, events, types).
2. Run `pnpm tauri-typegen` to regenerate the bindings under `src/generated/`.
3. Clean up the generated files: remove unused imports and symbols introduced by the generator while preserving all types that are actually exported/used.

## Cleaning Generated Files

The generator commonly emits unused imports and unused declarations. Fix these by trimming the code, not by relaxing the type checker:

- **Remove unused imports** — delete names from import lists (or drop the import entirely if nothing from that module is used).
- **Remove unused declarations** — drop exported types/interfaces/functions that the generator emitted but nothing references.
- **Preserve all used symbols** — keep every type that is actually consumed by callers.
- **Do not disable TypeScript restrictions or lint rules** to suppress the warnings. No `// @ts-ignore`, `// @ts-expect-error`, `eslint-disable` comments, no `any` casts, and no loosening of `tsconfig`/eslint config to paper over generated noise.
- **Do not reformat unrelated code** in the generated files beyond the minimal edits needed to remove unused symbols.

## Acceptance

After cleanup, the generated files must:

- Pass `pnpm build` (which runs `tsc`).
- Pass the project's lint rules with no suppressions added.
