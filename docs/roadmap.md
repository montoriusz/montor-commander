# Roadmap

## Features

- Terminal-chat view modes: Balloons, Split-view, Notebook.
- UI options — colors and fonts (scope TBD: theme presets vs. full custom theming).
- Command-line guard — context-independent, security-focused review of LLM-suggested and user-entered commands:
  - On demand or always on (Guardian mode).
  - Potential loss rating indicator.
- Enhanced system information in the system prompt (OS, shell, cwd).
- Shell command discovery tools.
- Silent filesystem traversal, file reading, and other safe read-only tools (keeping terminal space for critical operations only).
- Switch between different LLM models and providers
- User-friendly command output representation (Markdown/HTML)
- LLM-generated forms for user input
- Session management:
  - Switch, edit (remove unwanted turns), or suspend chat sessions within the same terminal session.
  - Restore sessions.
- Attachment/summarization pipeline to keep large terminal output out of the LLM context.

## Non-goals

- Acting as a full shell or replacing the user's terminal emulator.
- IDE features (file editing, syntax highlighting, debugging).
- Automatic agent mode.
-
