# smpte-to-midi

Decodificador de LTC (SMPTE) a MIDI y OSC desarrollado con Tauri. Permite disparar eventos en tiempos específicos basados en una entrada de audio con código de tiempo.

## Requisitos

- Rust y Cargo
- Node.js y npm

## Instalación

1. Clonar el repositorio.
2. Instalar las dependencias de Node:
   ```bash
   npm install
   ```

## Uso

Para ejecutar la aplicación en modo desarrollo:
```bash
npm run tauri dev
```

Para compilar la versión de producción:
```bash
npm run tauri build
```

## Características

- Decodificación de audio LTC en tiempo real.
- Soporte para múltiples frame rates (23.976, 24, 25, 29.97, 30).
- Envío de mensajes MIDI (Note, CC) y OSC.
- Gestión de hotcues (triggers) con edición inline estilo Excel.
- Importación y exportación de configuraciones.
