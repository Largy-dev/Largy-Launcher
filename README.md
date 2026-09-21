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

## Authentification Microsoft

La connexion utilise le flux OAuth « device code » (`consumers` tenant), sans secret client, avec
l'application Azure de Largy Launcher **déjà intégrée** : rien à configurer, connecte-toi simplement
avec ton compte Microsoft depuis l'écran de connexion.

Cette application est en cours d'approbation par Microsoft pour l'API Minecraft Services (obligatoire
depuis peu pour toute nouvelle application Azure — voir [aka.ms/mce-reviewappid](https://aka.ms/mce-reviewappid)).
Tant que l'approbation n'est pas passée, la connexion Microsoft échoue avec une erreur 403 : utilise le
**Mode Hors-ligne** (Paramètres) en attendant — il permet de lancer le jeu avec un profil local, sur du
solo ou un serveur explicitement configuré en mode hors-ligne (pas sur les serveurs officiels).

Si tu veux utiliser ta propre application Azure à la place (par exemple pour ton propre fork), tu peux
enregistrer la tienne sur [Entra ID](https://entra.microsoft.com) (App registrations → New registration,
avec « Allow public client flows » activé) et éditer `azure_client_id` directement dans le fichier
`settings.json` du launcher.

## Modpacks CurseForge

FTB fonctionne sans aucune configuration. Pour parcourir et installer des modpacks **CurseForge**, il
faut une clé API personnelle, gratuite :

1. Va sur [console.curseforge.com](https://console.curseforge.com/) et crée un compte / connecte-toi.
2. Génère une clé API (section **API Keys**).
3. Colle-la dans Largy Launcher → Paramètres → Comptes & API → Clé API CurseForge.

L'onglet CurseForge apparaît automatiquement dans Modpacks dès qu'une clé valide est enregistrée —
sans clé, seul FTB est affiché. Cette clé est personnelle : ne la partage pas, les conditions
d'utilisation de CurseForge interdisent de la distribuer (voir leurs
[conditions d'utilisation de l'API](https://support.curseforge.com/support/solutions/articles/9000207405)).
