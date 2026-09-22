# Largy Launcher

![CI](https://github.com/Largy-dev/Largy-Launcher/actions/workflows/ci.yml/badge.svg)

### [⬇️ Télécharger la dernière version](https://github.com/Largy-dev/Largy-Launcher/releases/latest)

Un launcher Minecraft personnel pour Windows : authentification Microsoft, modpacks FTB/CurseForge,
mod loaders Fabric/Quilt/Forge/NeoForge, gestion multi-instances. Construit avec Tauri (Rust) +
React/TypeScript.

## Installer (pour jouer, pas pour développer)

1. Va sur la [page Releases](https://github.com/Largy-dev/Largy-Launcher/releases/latest) (lien
   ci-dessus).
2. Télécharge le fichier `Largy Launcher_x.y.z_x64-setup.exe`.
3. Lance-le et suis l'installateur.
4. Ouvre Largy Launcher, connecte-toi avec ton compte Microsoft (aucune configuration requise,
   l'application Azure est déjà intégrée dans le launcher).

Si Windows SmartScreen affiche un avertissement (l'exécutable n'est pas signé avec un certificat
payant), clique sur **Informations complémentaires → Exécuter quand même**.

## Fonctionnel aujourd'hui

- **Connexion Microsoft** : device code → Xbox Live → XSTS → Minecraft Services, sans rien à
  configurer. Refresh token stocké de façon sécurisée (Gestionnaire d'identification Windows).
- **Mode Hors-ligne** : profil local (pseudo + UUID déterministe) pour jouer sans compte Microsoft,
  en solo ou sur un serveur explicitement configuré en mode hors-ligne.
- **Modpacks FTB** : recherche, détails, installation — aucune clé requise.
- **Modpacks CurseForge** : idem, avec une clé API personnelle gratuite (voir plus bas).
- **Mod loaders** : Fabric, Quilt, Forge, NeoForge (y compris l'exécution de la chaîne de
  processeurs des installeurs Forge/NeoForge).
- **Instances multiples** : chaque instance est un dossier isolé (mods/saves/config/resourcepacks/
  natives), avec ses propres réglages mémoire/JVM.
- **Mise à jour de modpack en place** : installer une nouvelle version d'un modpack déjà présent
  met à jour l'instance existante (mods/overrides obsolètes supprimés, saves et ajouts manuels
  jamais touchés) au lieu de créer une instance en double.
- **Gestion des mods** : activer/désactiver, supprimer ou ajouter un `.jar` directement dans une
  instance moddée, sans quitter le launcher.
- **Téléchargements** : moteur concurrent avec vérification de checksum (sha1) et reprise
  intelligente (ne retélécharge rien de déjà valide). Les fichiers qu'un modpack ne peut pas
  fournir automatiquement (restriction posée par l'auteur sur CurseForge) sont listés dans une
  fenêtre dédiée, avec lien direct de téléchargement et accès au dossier de l'instance.
- **Java automatique** : détection du runtime requis par version de Minecraft et téléchargement
  du binaire Mojang correspondant (pas besoin d'installer Java soi-même).
- **Lancement** : logs (stdout/stderr) et progression de téléchargement diffusés en direct dans
  l'interface, avec détection des causes de crash les plus courantes (mémoire insuffisante,
  incompatibilité de mods, version Java...) affichée directement à l'écran.
- **Thème** : couleur d'accent personnalisable (vert/noir/blanc/violet/rouge/bleu).
- **Mise à jour automatique** : vérification au démarrage (et à la demande dans Paramètres) ; la
  nouvelle version se télécharge, s'installe et relance le launcher en un clic, via l'updater
  officiel Tauri (artefacts signés, voir [Développement](#développement)).
- **CI/CD** : tests + lint automatiques sur chaque push ; un tag `vX.Y.Z` déclenche un build, une
  release GitHub et la publication de la mise à jour automatique (voir [Développement](#développement)).

## À faire / connu

- **Approbation Microsoft en attente** : l'application Azure du launcher doit être validée par
  Microsoft pour l'API Minecraft Services (nouvelle exigence pour toute app tierce) ; en attendant,
  la connexion Microsoft peut échouer avec une erreur 403 — utiliser le Mode Hors-ligne.
- **CurseForge nécessite une clé par utilisateur** : choix délibéré (leurs conditions d'utilisation
  interdisent de distribuer une clé partagée), pas un bug.
- **Certains mods CurseForge restent à télécharger manuellement** : quand l'auteur désactive la
  redistribution tierce, ce n'est pas qu'un champ d'API caché — CurseForge bloque aussi l'accès
  direct côté CDN. Pas de contournement possible côté launcher ; la fenêtre d'avertissement donne
  le lien direct pour le faire à la main.
- **Windows uniquement** : pas testé/empaqueté pour macOS ou Linux à ce stade.
- **Couverture de tests** : bonne sur la logique pure côté backend (parsing de manifestes,
  résolution de versions, providers, auth hors-ligne, orchestration de lancement — 100 tests) et sur
  les points d'intégration Tauri côté frontend (23 tests Vitest), mais la chaîne réseau complète de
  l'authentification Microsoft n'est pas testée automatiquement (nécessiterait de mocker plusieurs
  API externes).

## Stack technique

| Techno | Rôle |
|---|---|
| **Tauri v2** | Framework qui empaquette le frontend web dans une fenêtre native légère, avec un backend Rust pour tout ce qui touche au système (fichiers, process, réseau bas niveau). |
| **Rust** (`src-tauri/`) | Backend : auth, téléchargements, gestion des instances/Java/mod loaders, lancement du jeu. |
| **React 19 + TypeScript** (`src/`) | Interface utilisateur. |
| **Vite** | Bundler/dev server du frontend. |
| **Tailwind CSS v4 + shadcn/ui** | Styles utilitaires + composants d'interface (boutons, dialogues, menus...) accessibles et personnalisables. |
| **TanStack Query** | Cache et synchronisation des données venant du backend Rust (instances, modpacks, réglages). |
| **Zustand** | État global léger côté frontend (compte actif, logs de lancement, progression). |
| **react-router** | Navigation entre écrans. |
| **reqwest / tokio** (Rust) | Requêtes HTTP asynchrones vers les API Mojang/Microsoft/FTB/CurseForge. |
| **keyring** (Rust) | Stockage sécurisé du refresh token Microsoft dans le Gestionnaire d'identification Windows. |

## Comment ça marche

**Authentification** (`src-tauri/src/auth/`) : flux OAuth "device code" Microsoft (pas de secret
client) → jeton Xbox Live → autorisation XSTS → jeton Minecraft Services → profil joueur. Le refresh
token est gardé en sécurité pour reconnecter automatiquement au démarrage.

**Instances** (`src-tauri/src/instances/`) : chaque installation Minecraft est un dossier autonome
avec son `instance.json` (version, mod loader, réglages mémoire...) — aucun état partagé entre
instances, à part les caches communs (bibliothèques, assets, runtimes Java) pour éviter de
retélécharger la même chose plusieurs fois.

**Résolution de version** (`src-tauri/src/minecraft/`) : lit le manifeste officiel Mojang pour une
version donnée, télécharge le client, ses bibliothèques et ses assets, puis assemble la ligne de
commande Java finale (`launch_args.rs`).

**Mod loaders** (`src-tauri/src/modloaders/`) : Fabric et Quilt exposent une simple "delta" JSON à
fusionner par-dessus le manifeste vanilla. Forge et NeoForge sont plus complexes — ils fournissent un
`.jar` installeur qui doit exécuter une chaîne de "processeurs" Java pour patcher le client ; c'est la
partie la plus délicate du launcher (`forge_common.rs`).

**Providers de modpacks** (`src-tauri/src/providers/`) : abstraction commune (`ModpackProvider`)
implémentée pour FTB (API publique, sans clé) et CurseForge (clé API personnelle requise). Installer
un modpack résout sa version, télécharge chaque fichier, et crée une nouvelle instance préconfigurée.

**Lancement** (`src-tauri/src/launch/`) : assemble compte actif + version résolue + mod loader +
runtime Java dans une commande, lance le processus, et relaie ses logs vers l'interface en direct.

## Développement

```bash
npm install
npm run tauri dev
```

> Sur Linux (y compris WSL), `tauri dev` nécessite les paquets système webkit2gtk/rsvg2 (voir
> [prérequis Tauri](https://tauri.app/start/prerequisites/)). La cible finale de ce projet est Windows.

### Build local

```bash
npm run tauri build
```

Produit `src-tauri/target/release/bundle/nsis/*.exe` et `bundle/msi/*.msi`.

### Tests & lint

```bash
cd src-tauri && cargo test && cargo clippy --all-targets
npx tsc --noEmit && npm run lint && npm run format:check && npm test
```

### Release automatisée

Pousser un tag `vX.Y.Z` déclenche `.github/workflows/release.yml` : build complet sur un runner
Windows, puis publication directe d'une **release GitHub** avec les installeurs (`.exe`, `.msi`)
attachés, plus `latest.json` (manifeste + signature) pour l'auto-update :

```bash
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

N'oublie pas de bumper la version dans `package.json`, `src-tauri/Cargo.toml` et
`src-tauri/tauri.conf.json` avant de tagger — c'est cette version qui est comparée par l'updater.

Chaque push sur `main` (et chaque pull request) déclenche aussi `.github/workflows/ci.yml`
(`cargo test`, `cargo clippy -D warnings`, `tsc --noEmit`) pour attraper les régressions avant même
de tagger une release.

### Auto-update — détails

Basé sur `tauri-plugin-updater` : chaque release signe ses artefacts avec une clé privée (secret
GitHub Actions `TAURI_SIGNING_PRIVATE_KEY`, jamais dans le dépôt) et publie un `latest.json` que le
launcher interroge via l'URL stable `github.com/.../releases/latest/download/latest.json`. La clé
publique correspondante vit dans `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`) — normal
qu'elle soit visible, une clé publique n'a rien à cacher.

## Structure

- `src-tauri/src/auth/` — authentification Microsoft → Xbox Live → XSTS → Minecraft Services
- `src-tauri/src/instances/` — gestion des instances (dossiers isolés) et de leurs mods (`mods.rs`)
- `src-tauri/src/minecraft/` — résolution de version, assets, bibliothèques, arguments de lancement
- `src-tauri/src/modloaders/` — Fabric, Quilt, Forge, NeoForge
- `src-tauri/src/providers/` — abstraction des sources de modpacks (FTB, CurseForge)
- `src-tauri/src/java/` — gestion des runtimes Java
- `src-tauri/src/download/` — moteur de téléchargement concurrent
- `src-tauri/src/launch/` — assemblage de la commande, lancement du process, détection de crash
  (`crash_detect.rs`)
- `src-tauri/src/util/` — petits utilitaires partagés (substitution de placeholders)
- `src/screens/` — écrans de l'application (Login, Instances, Mods d'instance, Modpacks,
  Paramètres, Lancement...)

## Authentification Microsoft — détails

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

## Modpacks CurseForge — détails

FTB fonctionne sans aucune configuration. Pour parcourir et installer des modpacks **CurseForge**, il
faut une clé API personnelle, gratuite :

1. Va sur [console.curseforge.com](https://console.curseforge.com/) et crée un compte / connecte-toi.
2. Génère une clé API (section **API Keys**).
3. Colle-la dans Largy Launcher → Paramètres → Comptes & API → Clé API CurseForge.

L'onglet CurseForge apparaît automatiquement dans Modpacks dès qu'une clé valide est enregistrée —
sans clé, seul FTB est affiché. Cette clé est personnelle : ne la partage pas, les conditions
d'utilisation de CurseForge interdisent de la distribuer (voir leurs
[conditions d'utilisation de l'API](https://support.curseforge.com/support/solutions/articles/9000207405)).
