# Bellreader

A Windows and Android app for guided developmental observations based on **Kuno Bellers Entwicklungstabelle 0–9** (Simone Beller, 2016).

The observer enters the child's age in months and answers one condition at a time. For each of the eight developmental areas, Bellreader finds the floor, ceiling and total score, then shows the result as an answer table and a development profile chart.

Built with [Tauri 2](https://tauri.app): a Rust backend with a plain HTML, CSS and JavaScript frontend.

## Features

- Guided questionnaire: each condition is shown with its example and four answers: **Ja**, **Teilweise**, **Nein**, **Weiß ich nicht**
- Starting phase picked automatically from the child's age (one phase below the expected phase)
- Floor, ceiling and total score for each of the 8 areas
- Answer table per area: Tut es / Tut es teilweise / Tut es nicht / Weiß ich nicht, plus phase values
- Profile chart showing floor, ceiling, total score and the median of all total scores
- Works offline: the full table is compiled into the app
- Light and dark mode, answer buttons stay fixed at the bottom, keys `1`–`4` answer on desktop

## Developmental areas

1. Körperbewusstsein & -pflege
2. Umgebungsbewusstsein
3. Sozial-emotionale Entwicklung
4. Spieltätigkeit
5. Sprache & Literacy
6. Kognition
7. Grobmotorik
8. Feinmotorik

Each area has 18 phases.

## How the assessment works

### Starting phase

The expected phase comes from the child's age (from the appendix "Phasen und chronologisches Alter"):

| Phase | From month | Phase | From month | Phase | From month |
|---|---|---|---|---|---|
| 1 | 0 | 7 | 24 | 13 | 60 |
| 2 | 3 | 8 | 30 | 14 | 66 |
| 3 | 6 | 9 | 36 | 15 | 72 |
| 4 | 9 | 10 | 42 | 16 | 84 |
| 5 | 12 | 11 | 48 | 17 | 96 |
| 6 | 18 | 12 | 54 | 18 | 108 |

The assessment starts one phase below the expected phase, never below phase 1.

### Scoring

| Answer | Points |
|---|---|
| Ja | 1 |
| Teilweise | 0.5 |
| Nein | 0 |
| Weiß ich nicht | not counted |

**Phase value** = points ÷ number of counted answers.

### Floor, ceiling and total score

For each area:

- **Floor (Boden):** the last phase answered fully with Ja (phase value 1), directly below the first phase that is not fully Ja. If phase 1 is not fully Ja, the floor is 0.
- **Ceiling (Decke):** the first phase above the floor answered fully with Nein (phase value 0). If there is none up to phase 18, the ceiling is 18.
- **Total score:** floor + the phase values of every phase above the floor, up to and including the ceiling.

Example: floor 5, phase 6 = 0.75, phase 7 = 0.25, phase 8 = 0 (ceiling) → total score **6.0**.

Rules:

- Every phase the search reaches is asked in full, so no phase is counted without being observed.
- A phase answered only with "Weiß ich nicht" has no value and is skipped.
- The median line in the chart is the median of the eight total scores.

## Project structure

```
Bellreader/
├── src/                          Frontend
│   ├── index.html                Screens: age input, question, results
│   ├── main.js                   UI logic, chart and table
│   └── styles.css
└── src-tauri/                    Rust backend
    ├── data/
    │   ├── entwicklungstabelle.example.json   Placeholder data (in the repo)
    │   └── entwicklungstabelle.json           Real data (not in the repo)
    ├── src/
    │   ├── lib.rs                App setup and command registration
    │   ├── table.rs              Loads the JSON
    │   └── assessment.rs         Age table, search, scoring, tests
    ├── icons/                    App icons
    ├── Cargo.toml
    └── tauri.conf.json
```

### Data

The real table is **not included** in this repository because its content is protected by copyright (see [License and data](#license-and-data)). The app compiles the data into the binary, so you need a data file before you can build:

- **To try the app:** copy the placeholder file. It has the full structure (8 areas × 18 phases) with made-up conditions:
  ```powershell
  Copy-Item src-tauri\data\entwicklungstabelle.example.json src-tauri\data\entwicklungstabelle.json
  ```
- **For real use:** create `src-tauri/data/entwicklungstabelle.json` from your own licensed copy of *Kuno Bellers Entwicklungstabelle 0–9*, in the format below.

`entwicklungstabelle.json` is listed in `.gitignore` so the real data is never committed.

### Data format

The file is a list of areas, each containing phases, each containing conditions:

```json
[
  {
    "name": "Grobmotorik",
    "phases": [
      {
        "number": 1,
        "conditions": [
          { "number": 1, "title": "Strampelt mit Armen und Beinen.", "example": "" }
        ]
      }
    ]
  }
]
```

### Commands called by the frontend

| Command | Arguments | Returns |
|---|---|---|
| `start_assessment` | `{ ageMonths }` | First question |
| `answer` | `{ answer: "ja" \| "teilweise" \| "nein" \| "weiss_nicht" }` | Next question, or the results when finished |
| `get_areas` | – | Names of all areas |
| `get_phase` | `{ area, phase }` | One phase with its conditions |
| `get_table` | – | The whole table |

## Setup (Windows)

1. Install [Rust](https://rustup.rs), including the Visual Studio C++ Build Tools ("Desktop development with C++").
2. Install the Tauri CLI:
   ```powershell
   cargo install tauri-cli --version "^2"
   ```
3. Recommended VS Code extensions: [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) and [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode).

WebView2 is already included in Windows 10 and 11.

## Development

Run the app with live reload:

```powershell
cargo tauri dev
```

Run the tests from `src-tauri`:

```powershell
cd src-tauri
cargo test
```

The tests check that the data loads (8 areas × 18 phases), the age table, scoring, floor and ceiling from different starting ages, the edge cases, the answer table and the median.

## Building for Windows

```powershell
cargo tauri build
```

Output in `src-tauri\target\release\`:

| File | Description |
|---|---|
| `bellreader.exe` | Portable app, runs without installing |
| `bundle\nsis\*-setup.exe` | Installer |
| `bundle\msi\*.msi` | MSI installer |

Add `--bundles nsis` to build only the setup.exe. The app is not code-signed, so Windows SmartScreen may show an "unknown publisher" warning.

## Android

Additional requirements: Android Studio with the SDK, NDK, Build-Tools and Command-line Tools, plus these environment variables:

```powershell
[System.Environment]::SetEnvironmentVariable("JAVA_HOME", "C:\Program Files\Android\Android Studio\jbr", "User")
[System.Environment]::SetEnvironmentVariable("ANDROID_HOME", "$env:LocalAppData\Android\Sdk", "User")
$VERSION = Get-ChildItem -Name "$env:LocalAppData\Android\Sdk\ndk" | Select-Object -Last 1
[System.Environment]::SetEnvironmentVariable("NDK_HOME", "$env:LocalAppData\Android\Sdk\ndk\$VERSION", "User")
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

Turn on Windows Developer Mode, restart the terminal, then:

```powershell
cargo tauri android init
cargo tauri android dev
cargo tauri android build
```

A physical phone with USB debugging is usually faster and more reliable than the emulator.

## App icon

The icon is a flat bell on green. To regenerate all icon sizes from one 1024×1024 PNG:

```powershell
cargo tauri icon bellreader-icon.png
```

Run it again after `cargo tauri android init` so the Android icons are created too.

## License and data

The app's source code is the author's own work.

The developmental table, age ranges and scoring method come from *Kuno Bellers Entwicklungstabelle 0–9* © Simone Beller (2016). The table's text is protected by copyright and is therefore not part of this repository; only placeholder data is included. Do not publish the real data or distribute an app containing it (for example in an app store) without permission from the rights holder.
