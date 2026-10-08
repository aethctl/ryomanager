# RyoManager

RyoManager is a Ryoku-native system task manager for Linux. It uses the visual language of Ryoku Hub and Ryotunes while following the information hierarchy of a desktop task manager rather than a terminal monitor.

The goal is simple: make processes and performance understandable at a glance, then expose deeper controls without turning the interface into a glorified `top` frontend.

## Current foundation

The current build includes:

- processes grouped the way a desktop sees them: Apps (anything that owns a window), Background and System, with every launch of one application merged under one row that expands to its processes
- group identity from the systemd unit and the desktop entry, with names and icons from the entry and descriptions from the unit
- per-process CPU, private memory, disk throughput, GPU engine use, an energy estimate, state, PID, threads, user, nice, start time, unit, open sockets and swap, with a column chooser
- an inspector with exact memory (PSS and its composition), open files, context switches, OOM score, windows, members and actions: End task, Force stop, Suspend and Resume, priority, Open location, Copy command, every one tied to the process identity (PID plus start time) so a recycled PID is never acted on
- a Performance page with a resource rail (CPU, memory, each disk, each network interface, each GPU, energy, thermals), 60-sample graphs with gaps where sampling paused, per-logical-processor view, memory and swap composition, disk and network link details, GPU memory and codec engines, battery and package power, sensor tables, and the processes driving each resource
- readings the system refuses (another user's counters, root-only power counters) shown as unavailable with the reason, never as zero
- a selectable refresh interval: live, 1 minute or 3 minutes
- a system identity page for hostname, OS and kernel
- Ryoku/Ryotunes-inspired monochrome application UI

Startup applications and services are planned as first-class pages.

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
