# RyoManager

RyoManager is a Ryoku-native system task manager for Linux. It uses the visual language of Ryoku Hub and Ryotunes while following the information hierarchy of a desktop task manager rather than a terminal monitor.

The goal is simple: make processes and performance understandable at a glance, then expose deeper controls without turning the interface into a glorified `top` frontend.

## Current foundation

The first working build includes:

- live process list with 1-second refresh
- search by process name, command or PID
- sorting by name, PID, CPU and memory
- safe End Task (`SIGTERM`) and Force Stop (`SIGKILL`) actions
- live CPU and memory summaries
- 60-second CPU and memory performance graphs
- CPU model, core and logical processor information
- memory, availability and swap information
- system identity page for hostname, OS and kernel
- Ryoku/Ryotunes-inspired monochrome application UI

Startup applications, services, process grouping, GPU, disk, network and deeper process inspection are planned as first-class pages rather than bolt-on terminal widgets.

## Development

RyoManager uses Tauri 2, Rust, Svelte 5 and Vite.

```sh
nix develop
npm install
npm run tauri -- dev
```

Frontend checks:

```sh
npm run check
npm run build
```

Rust backend check:

```sh
nix develop -c cargo check --manifest-path src-tauri/Cargo.toml
```

## Design principles

- readable before technical
- one-pixel structure and restrained Ryoku surfaces
- no rainbow resource dashboard
- no permanent decorative animation
- process actions should be understandable without knowing Unix signals
- polling cost must stay small enough that opening the task manager does not become the workload
- advanced detail belongs behind progressive disclosure, not in the default view

## Status

RyoManager is early development software. The current build establishes the native app shell, live process backend and first performance views.
