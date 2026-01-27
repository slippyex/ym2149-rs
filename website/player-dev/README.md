# YM2149 Web Player (Development)

TypeScript-based web player for YM2149 chiptunes with Vite build system.

## Projektstruktur

```
website/
├── player-dev/              # Quellcode (in Git)
│   ├── src/                 # TypeScript Source
│   ├── pkg/                 # WASM Module
│   ├── collections/         # Musik-Sammlungen
│   │   ├── arkos/
│   │   ├── ProjectAY/
│   │   ├── sndh_lf_2026/
│   │   └── ym/
│   ├── index.dev.html       # HTML Template
│   ├── catalog*.json*       # Track-Katalog
│   └── ...
└── player/                  # Build Output (gitignored)
    ├── index.html
    ├── *.js, *.css, *.wasm
    ├── catalog*.json*
    ├── arkos/               # ← kopiert aus collections/
    ├── ProjectAY/
    ├── sndh_lf_2026/
    └── ym/
```

## Setup

```bash
cd website/player-dev
npm install
```

## Development (Port 5173)

```bash
npm run dev
```

Öffnet http://localhost:5173/index.dev.html mit Hot Reload.

## Type Check

```bash
npm run typecheck
```

## Production Build

```bash
npm run build
```

Erstellt alle Dateien in `../player/`:
- Baut TypeScript → JavaScript (minified)
- Kopiert WASM Module
- Kopiert Katalog-Dateien
- Kopiert Collections (arkos, ProjectAY, sndh_lf_2026, ym)
- Benennt index.dev.html → index.html

## Preview (Port 4173)

```bash
npm run preview
```

Testet den Production Build aus `../player/`.

## Clean

```bash
npm run clean
```

Löscht das komplette `../player/` Verzeichnis.

---

## Production Deployment

### 1. Build erstellen

```bash
cd website/player-dev
npm install
npm run build
```

### 2. Website deployen

Nach dem Build enthält `/website` alle benötigten Dateien:

```
website/
├── index.html              # Hauptseite
├── downloads.html
├── tutorials.html
├── robots.txt
├── sitemap.xml
├── assets/
├── screenshots/
└── player/                 # Web Player (Build Output)
    ├── index.html
    ├── index.dev.*.js      # ~64 KB
    ├── index.*.css         # ~20 KB
    ├── ym2149_wasm.*.js    # ~7 KB
    ├── ym2149_wasm_bg.*.wasm # ~1 MB
    ├── catalog.json.gz     # ~5 MB
    ├── catalog.min.json    # ~11 MB
    ├── catalog.json        # ~26 MB
    ├── arkos/              # Arkos Tracker Songs
    ├── ProjectAY/          # ZX Spectrum AY Music
    ├── sndh_lf_2026/       # Atari ST SNDH Archive
    └── ym/                 # YM Format Songs
```

### 3. Auf Server kopieren

```bash
# Komplettes website/ kopieren, OHNE player-dev/
rsync -av --exclude='player-dev' website/ server:/path/
```

### 4. Server-Konfiguration

MIME-Types:
- `.wasm` → `application/wasm`
- `.js` → `application/javascript`
- `.gz` → `application/gzip` (optional)
- `.sndh`, `.ym`, `.ay`, `.aks` → `application/octet-stream`

---

## NPM Scripts

| Script | Beschreibung |
|--------|--------------|
| `npm run dev` | Vite Dev Server (Port 5173) |
| `npm run build` | Production Build → `../player/` |
| `npm run preview` | Preview Production Build |
| `npm run typecheck` | TypeScript Check |
| `npm run clean` | Löscht `../player/` |

---

## Dateistruktur (Development)

```
player-dev/
├── src/
│   ├── main.ts              # Entry Point
│   ├── types/               # TypeScript Definitionen
│   ├── audio/               # Audio Context & Playback
│   ├── ui/                  # UI Komponenten
│   ├── visualization/       # Canvas Visualisierung
│   └── styles/              # CSS (Tailwind)
├── pkg/                     # WASM (wasm-bindgen)
├── collections/             # Musik-Sammlungen
│   ├── arkos/
│   ├── ProjectAY/
│   ├── sndh_lf_2026/
│   └── ym/
├── index.dev.html           # HTML Template
├── catalog.json.gz          # Track-Katalog
├── vite.config.ts
├── tsconfig.json
└── package.json
```
