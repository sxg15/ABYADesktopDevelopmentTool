# ABYA Desktop Development Tool

Portable Windows tooling for ABYA game development tasks, managed game
instances, structured runtime logs, and future LLM/MCP workflows.

## Commands

- `npm run tauri dev`: run the desktop application.
- `npm run check`: validate module Skills, changelog policy, tests, and frontend build.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust tests.
- `npm run build:portable`: build the release executable into `Publish`.

Read `AGENTS.md`, `ARCHITECTURE.md`, and the relevant module Skill before
changing production code.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
