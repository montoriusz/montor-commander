# Roadmap

## Features

- Terminal-chat view modes: Balloons, Split-view, Notebook.
- Switch between different LLM models and providers
- UI options — colors and fonts (scope TBD: theme presets vs. full custom theming, automatic system colors theme).
- Command-line guard — context-independent, security-focused review of LLM-suggested and user-entered commands:
  - On demand or always on (Guardian mode).
  - Potential loss rating indicator.
- Extract CWD to `cwd` attribute of `<prompt>`.
- Tools:
  - Shell command discovery tools.
  - Silent filesystem traversal, file reading, and other safe read-only tools (keeping terminal space for critical operations only).
- User-friendly command output representation (Markdown/HTML)
- Utilize OSC 3008 for terminal session metadata.
- LLM-generated forms for user input
- Session management:
  - Switch, edit (remove unwanted turns), or suspend chat sessions within the same terminal session.
  - Restore sessions.
- Attachment/summarization pipeline to keep large terminal output out of the LLM context.

## Non-goals

- Acting as a full shell or replacing the user's terminal emulator.
- IDE features (file editing, syntax highlighting, debugging).
- Automatic agent mode.

## Known Issues

- Processes don't exit cleanly
-
