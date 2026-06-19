# AGENTS.md

## Overview

A Tauri 2 desktop terminal app. The frontend is TypeScript + Vite using xterm.js; the backend is Rust in `src-tauri`.

## Layout

- `src/` — frontend TypeScript + React, with generated Tauri bindings in `src/generated/`.
  - `src/app/` — app components (`app.tsx`) and hooks (`use-terminal.ts`, `terminal-sections.ts`).
  - `ui/` - Pure UI components, isolated from business logic.
    - `primitives/` - Panda CSS / Ark UI primitive components.
    - `composites/` - Composites and domain-specific components.
    - `layouts/` - Generic slot-based containers for pages and their sections.
    - `hooks/` - UI component companion hooks, isolated from business logic.
  - `stories/` - Storybook stories for UI components, mock-ups and other examples.
  - `theme/` + `panda.config.ts` - Panda CSS theme configuration.
  - `src/theme/` — Panda CSS theme configuration.
  - `src/main.tsx` — React entry point.
- `src-tauri/` — Rust backend (`src/`, `Cargo.toml`, `tauri.conf.json`).
- `index.html` — Vite entry point.

## Commands

Uses pnpm.

- `pnpm install` — install dependencies.
- `pnpm tauri dev` — run the app in development.
- `pnpm build` — type-check and build the frontend (`tsc && vite build`).
- `pnpm tauri build` — build the desktop app.
- `pnpm tauri-typegen` — regenerate Tauri type bindings.

## Conventions

- Save Implementation plans in `src/implementation-plans`.
- Frontend is TypeScript + React; keep `tsc` passing.
- xterm.js for used as a terminal emulator.
- There is an assistant chat next to the terminal.
- Do not edit generated files in `src/generated/`; regenerate them instead.
- Styling uses Panda CSS (`styled-system/`) and Ark UI primitives — avoid raw CSS unless necessary.
- Backend is Rust; run `cargo check` in `src-tauri/`.
- In rust, write async code using `tokio` where it has advantages.

### File Naming (`src/`)

**Kebab Case** must be used for all file and directory names in `src/`. E.g. `my-function.ts`, `my-component/helpers.ts`) for everything that is not a React component name.
If a component needs to group more sub-components, private hooks, or other helpers, they should be placed in a folder. The folder should contain an index file re-exporting public symbols.
If more further grouping is needed, files that group multiple declarations of the same type/purpose can include type suffixes such as `constants`, `helpers`, or `types` in their names; e.g., `oidc.constants.ts`, `sub-component.helpers.ts`.

### File Naming (`src-tauri/`)

Prefer **Snake Case** for all file and directory names in `src-tauri/`. Adhere to Rust conventions.
