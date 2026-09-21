# Largy Launcher

Un launcher Minecraft personnel pour Windows (auth Microsoft, modpacks FTB, gestion d'instances multi-loaders), construit avec Tauri (Rust) + React/TypeScript.

## Stack

- **Backend** : Rust (`src-tauri/`), Tauri v2
- **Frontend** : React + TypeScript, Vite, Tailwind CSS, shadcn/ui

## Développement

```bash
npm install
npm run tauri dev
```

> Sur Linux (y compris WSL), `tauri dev` nécessite les paquets système webkit2gtk/rsvg2 (voir [prérequis Tauri](https://tauri.app/start/prerequisites/)). La cible finale de ce projet est Windows.

## Build

```bash
npm run tauri build
```

## Structure

- `src-tauri/src/auth/` — authentification Microsoft → Xbox Live → XSTS → Minecraft Services
- `src-tauri/src/instances/` — gestion des instances (dossiers isolés)
- `src-tauri/src/minecraft/` — résolution de version, assets, libraries, arguments de lancement
- `src-tauri/src/modloaders/` — Fabric, Quilt, Forge, NeoForge
- `src-tauri/src/providers/` — abstraction des sources de modpacks (FTB en premier)
- `src-tauri/src/java/` — gestion des runtimes Java
- `src-tauri/src/download/` — moteur de téléchargement concurrent
- `src/screens/` — écrans de l'application (Login, Instances, Modpacks, Paramètres...)

Voir le plan de développement complet pour le détail des phases.
