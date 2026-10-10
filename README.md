# RyoManager



## Development

RyoManager uses Wails 2, Go, Svelte 5 and Vite. The frontend lives in `frontend/`; the native Linux collector and process controls are implemented in Go.

```sh
nix develop
wails dev
```

Frontend checks:

```sh
cd frontend
npm ci
npm run check
npm run build
```

Go backend check:

```sh
go test ./...
```

Production build:

```sh
wails build
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
