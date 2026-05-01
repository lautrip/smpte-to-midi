# smpte-to-midi

LTC (SMPTE) to MIDI and OSC decoder built with Tauri. It allows triggering events at specific times based on an LTC audio input.

## Requirements

- Rust and Cargo
- Node.js and npm

## Installation

1. Clone the repository.
2. Install dependencies:
   ```bash
   npm install
   ```

## Usage

Run development mode:
```bash
npm run tauri dev
```

Build production version:
```bash
npm run tauri build
```

## Features

- Real-time LTC audio decoding.
- Supports multiple frame rates (23.976, 24, 25, 29.97, 30).
- Sends MIDI (Note, CC) and OSC messages.
- Hotcue management with Excel-style inline editing.
- Settings import and export.
