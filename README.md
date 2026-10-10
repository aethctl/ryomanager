# RyoManager

# RyoManager

<p align="center">
  <img src="readme/assets/2026_10_10_18_59_07_screenshot.png" width="100%" />
</p>

## Screenshots

<p align="center">
  <img src="readme/assets/2026_10_10_19_00_33_screenshot.png" width="49%" />
  <img src="readme/assets/2026_10_10_19_00_42_screenshot.png" width="49%" />
</p>

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
