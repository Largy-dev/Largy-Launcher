# Largy Launcher

![CI](https://github.com/Largy-dev/Largy-Launcher/actions/workflows/ci.yml/badge.svg)

Un launcher Minecraft personnel pour Windows : authentification Microsoft, modpacks FTB/CurseForge,
mod loaders Fabric/Quilt/Forge/NeoForge, gestion multi-instances. Construit avec Tauri (Rust) +
React/TypeScript.

## Installer (pour jouer, pas pour développer)

1. Va sur la [page Releases](https://github.com/Largy-dev/Largy-Launcher/releases/latest).
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
- **Téléchargements** : moteur concurrent avec vérification de checksum (sha1) et reprise
  intelligente (ne retélécharge rien de déjà valide).
- **Java automatique** : détection du runtime requis par version de Minecraft et téléchargement
  du binaire Mojang correspondant (pas besoin d'installer Java soi-même).
- **Lancement** : logs (stdout/stderr) et progression de téléchargement diffusés en direct dans
  l'interface.
- **Thème** : couleur d'accent personnalisable (vert/noir/blanc/violet/rouge/bleu).
- **CI/CD** : tests + lint automatiques sur chaque push ; un tag `vX.Y.Z` déclenche un build et une
  release GitHub automatiques (voir [Développement](#développement)).

## À faire / connu

- **Mise à jour automatique du client** : pas encore implémentée — il faut retélécharger et
  réinstaller manuellement une nouvelle version pour l'instant. (La release, elle, est déjà
  automatisée côté CI — il manque le mécanisme d'auto-update *dans* l'application.)
- **Mise à jour d'un modpack déjà installé** : installer une nouvelle version d'un modpack crée
  aujourd'hui une nouvelle instance à côté, plutôt que de mettre à jour l'instance existante en place.
- **Approbation Microsoft en attente** : l'application Azure du launcher doit être validée par
  Microsoft pour l'API Minecraft Services (nouvelle exigence pour toute app tierce) ; en attendant,
  la connexion Microsoft peut échouer avec une erreur 403 — utiliser le Mode Hors-ligne.
- **CurseForge nécessite une clé par utilisateur** : choix délibéré (leurs conditions d'utilisation
  interdisent de distribuer une clé partagée), pas un bug.
- **Windows uniquement** : pas testé/empaqueté pour macOS ou Linux à ce stade.
- **Couverture de tests** : bonne sur la logique pure (parsing de manifestes, résolution de
  versions, providers, auth hors-ligne — 52 tests), mais la chaîne réseau complète de
  l'authentification Microsoft n'est pas testée automatiquement (nécessiterait de mocker
  plusieurs API externes).

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
npx tsc --noEmit
```

### Release automatisée

Pousser un tag `vX.Y.Z` déclenche `.github/workflows/release.yml` : build complet sur un runner
Windows, puis publication d'une **release GitHub en brouillon** avec les installeurs (`.exe`, `.msi`)
attachés automatiquement. Il ne reste plus qu'à relire les notes et cliquer sur *Publish* :

```bash
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

Chaque push sur `main` (et chaque pull request) déclenche aussi `.github/workflows/ci.yml`
(`cargo test`, `cargo clippy -D warnings`, `tsc --noEmit`) pour attraper les régressions avant même
de tagger une release.

## Structure

- `src-tauri/src/auth/` — authentification Microsoft → Xbox Live → XSTS → Minecraft Services
- `src-tauri/src/instances/` — gestion des instances (dossiers isolés)
- `src-tauri/src/minecraft/` — résolution de version, assets, bibliothèques, arguments de lancement
- `src-tauri/src/modloaders/` — Fabric, Quilt, Forge, NeoForge
- `src-tauri/src/providers/` — abstraction des sources de modpacks (FTB, CurseForge)
- `src-tauri/src/java/` — gestion des runtimes Java
- `src-tauri/src/download/` — moteur de téléchargement concurrent
- `src-tauri/src/launch/` — assemblage de la commande et lancement du process
- `src/screens/` — écrans de l'application (Login, Instances, Modpacks, Paramètres, Lancement...)

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
