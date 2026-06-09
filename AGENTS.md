# AGENTS.md

## Overview

A Tauri 2 desktop terminal app. The frontend is TypeScript + Vite using xterm.js; the backend is Rust in `src-tauri`.

## Layout

- `src/` — frontend TypeScript (`main.ts`, `styles.css`), with generated Tauri bindings in `src/generated/`.
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

- Frontend is TypeScript; keep `tsc` passing.
- Backend is Rust; run `cargo check` in `src-tauri/`.
- Do not edit generated files in `src/generated/`; regenerate them instead.
