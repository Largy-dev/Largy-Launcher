# Largy Launcher

![CI](https://github.com/Largy-dev/Largy-Launcher/actions/workflows/ci.yml/badge.svg)
![Version](https://img.shields.io/github/v/release/Largy-dev/Largy-Launcher?label=version)
![Windows](https://img.shields.io/badge/plateforme-Windows-0078d4)

**Un launcher Minecraft pour Windows, beau et simple, qui s'occupe de tout** : Java, mod loaders,
modpacks FTB et CurseForge, mémoire… Tu choisis ton instance, tu cliques sur **Jouer**.

### [⬇️ Télécharger Largy Launcher (Windows)](https://github.com/Largy-dev/Largy-Launcher/releases/latest/download/LargyLauncher-Setup.exe)

<sub>Toujours la dernière version · [toutes les versions](https://github.com/Largy-dev/Largy-Launcher/releases)</sub>

![Écran d'accueil de Largy Launcher](docs/screenshots/home.jpg)

## Pour les joueurs

### Installer

1. Télécharge **`LargyLauncher-Setup.exe`** avec le bouton ci-dessus.
2. Lance-le et suis l'installateur (quelques secondes).
3. Ouvre Largy Launcher et connecte-toi avec ton compte Microsoft — rien d'autre à configurer.

Le launcher se **met à jour tout seul** : quand une nouvelle version sort, il te le propose au
démarrage et l'installe en un clic.

> **Windows affiche « Windows a protégé votre ordinateur » ?** C'est normal pour un logiciel
> indépendant sans certificat payant : clique sur **Informations complémentaires → Exécuter quand
> même**.

### Ce que tu peux faire

- **Jouer en un clic** — la grande bannière d'accueil te propose de reprendre ta dernière partie.
  Java est téléchargé automatiquement, dans la bonne version pour chaque Minecraft.
- **Installer des modpacks** — parcours les modpacks **FTB** (et **CurseForge** avec une clé
  gratuite, voir plus bas), installe-les en un clic, et mets-les à jour sans perdre tes mondes.
- **Créer tes propres instances** — Vanilla, Fabric, Quilt, Forge ou NeoForge, chacune dans son
  dossier, avec ses mods, ses mondes et ses réglages.
- **Gérer tes mods** — active, désactive, supprime ou ajoute un `.jar`, recherche dans des
  centaines de mods.
- **La bonne quantité de RAM** — le launcher calcule la mémoire conseillée pour chaque instance
  (selon le loader, le nombre de mods, la version et la RAM de ton PC) et te prévient si c'est trop
  peu ou trop.
- **Suivre ton lancement** — étapes en direct, vitesse de téléchargement, temps restant, puis RAM
  et processeur utilisés pendant la partie.
- **Comprendre un crash** — logs colorés et filtrables, et un diagnostic clair des causes
  fréquentes (mémoire insuffisante, mods incompatibles, mauvaise version de Java…).
- **Tes stats** — temps de jeu par instance et au total, dernière partie, nombre de mods.
- **Des notifications utiles** — fin d'installation, crash, fin de session, nouvelle version ;
  même quand tu es en jeu, via les notifications Windows (réglables).
- **À ton goût** — mode sombre, clair ou système, 9 couleurs d'accent ou ta propre couleur, fond
  flouté de ton modpack, taille de l'interface, animations complètes, réduites ou désactivées.
- **Mode Hors-ligne** — pour jouer sans compte Microsoft, en solo ou sur un serveur en mode
  hors-ligne.

### Aperçu

| | |
|---|---|
| ![Conseil de mémoire d'une instance](docs/screenshots/memory.jpg) | ![Paramètres d'apparence](docs/screenshots/settings.jpg) |
| **Conseil de RAM** adapté à chaque instance | **Personnalisation** appliquée en direct |
| ![Gestion des mods](docs/screenshots/mods.jpg) | ![Navigateur de modpacks](docs/screenshots/modpacks.jpg) |
| **Gestion des mods** avec recherche et filtres | **Modpacks** installables en un clic |

![Accueil en mode clair avec l'accent violet](docs/screenshots/light.jpg)

### Questions fréquentes

**La connexion Microsoft échoue avec une erreur 403.** L'application du launcher attend encore
l'approbation de Microsoft pour l'API Minecraft (obligatoire pour toute application tierce récente).
En attendant, active le **Mode Hors-ligne** dans Paramètres › Compte.

**Je ne vois que des modpacks FTB.** CurseForge demande une clé personnelle, gratuite :
1. Va sur [console.curseforge.com](https://console.curseforge.com/) et connecte-toi.
2. Génère une clé API (section **API Keys**).
3. Colle-la dans Paramètres › Avancé › Clé API CurseForge.

L'onglet CurseForge apparaît aussitôt. Cette clé est personnelle : ne la partage pas (les
[conditions de CurseForge](https://support.curseforge.com/support/solutions/articles/9000207405)
l'interdisent).

**Un modpack CurseForge dit que des fichiers sont à télécharger à la main.** Certains auteurs
interdisent le téléchargement par des launchers tiers. Le launcher liste ces fichiers avec un lien
direct et un bouton pour ouvrir le dossier de l'instance.

**Combien de RAM mettre ?** Regarde la pastille de couleur dans Réglages › Mémoire de l'instance :
vert, c'est bon. Le bouton **Appliquer** met directement la valeur conseillée.

---

## Pour les développeurs

Construit avec **Tauri 2** (backend Rust) et **React 19 + TypeScript** (interface).

### Lancer en local

```bash
npm install
npm run tauri dev
```

> Sur Linux (y compris WSL), `tauri dev` nécessite les paquets système webkit2gtk/rsvg2 (voir les
> [prérequis Tauri](https://tauri.app/start/prerequisites/)). La cible du projet est Windows.

Build local : `npm run tauri build` → `src-tauri/target/release/bundle/nsis/*.exe` et `bundle/msi/*.msi`.

### Tests & lint

```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
npx tsc --noEmit && npm run lint && npm run format:check && npm test
```

Couverture : la logique pure du backend (manifestes, résolution de versions, providers, auth
hors-ligne, instances, temps de jeu — 104 tests Rust) et du frontend (conseil RAM, parsing des logs,
formatage, préférences, notifications, préréglages JVM, appels Tauri — 61 tests Vitest). La chaîne
réseau complète de l'authentification Microsoft n'est pas testée automatiquement.

### Stack

| Techno | Rôle |
|---|---|
| **Tauri v2** | Fenêtre native légère autour du frontend web, backend Rust pour le système (fichiers, processus, réseau). |
| **Rust** (`src-tauri/`) | Auth, téléchargements, instances, Java, mod loaders, lancement, stats du processus (`sysinfo`). |
| **React 19 + TypeScript** (`src/`) | Interface. |
| **Vite** | Bundler et serveur de dev. |
| **Tailwind CSS v4 + shadcn/ui** | Styles et composants accessibles ; thème par tokens CSS (`src/index.css`). |
| **Motion** | Animations (transitions de pages, listes, compteurs), coupées selon la préférence de l'utilisateur. |
| **TanStack Query** | Cache des données du backend (instances, mods, modpacks, réglages). |
| **Zustand** | État global : runtime des instances, préférences visuelles (persistées), notifications. |
| **react-router** | Navigation entre écrans. |
| **reqwest / tokio** | HTTP asynchrone vers Mojang, Microsoft, FTB et CurseForge. |
| **keyring** | Refresh token Microsoft dans le Gestionnaire d'identification Windows. |
| **tauri-plugin-notification / -updater** | Notifications Windows natives, mise à jour automatique signée. |

### Comment ça marche

**Authentification** (`src-tauri/src/auth/`) : flux OAuth « device code » Microsoft (tenant
`consumers`, sans secret) → Xbox Live → XSTS → Minecraft Services → profil. Le refresh token permet
la reconnexion automatique, et une session presque expirée est rafraîchie avant le lancement.

**Instances** (`src-tauri/src/instances/`) : chaque instance est un dossier autonome avec son
`instance.json` (version, loader, mémoire, arguments JVM, temps de jeu…). Seuls les caches
(bibliothèques, assets, runtimes Java) sont partagés.

**Résolution de version** (`src-tauri/src/minecraft/`) : manifeste Mojang → client, bibliothèques,
assets → ligne de commande Java finale (`launch_args.rs`).

**Mod loaders** (`src-tauri/src/modloaders/`) : Fabric et Quilt fusionnent un profil JSON sur le
manifeste vanilla ; Forge et NeoForge exécutent la chaîne de processeurs de leur installeur
(`forge_common/`), la partie la plus délicate du launcher.

**Modpacks** (`src-tauri/src/providers/`) : trait commun `ModpackProvider` implémenté pour FTB
(`api.feed-the-beast.com`, sans clé) et CurseForge (clé personnelle). Une mise à jour ne supprime que
les fichiers installés par l'ancienne version du pack.

**Lancement** (`src-tauri/src/launch/`) : assemble compte + version + loader + Java, émet des
événements `launch-phase` à chaque étape, lance le processus, relaie ses logs, cumule le temps de jeu
à la sortie et analyse les crashs (`crash_detect.rs`). L'arrêt passe par un signal au lieu de
verrouiller le processus.

**Interface** (`src/`) : les événements du backend sont branchés une seule fois
(`hooks/useAppEvents.ts`) vers les stores ; `lib/notify.ts` est le point d'entrée unique des
notifications (toast + historique + notification Windows) ; `lib/ramAdvice.ts` calcule le conseil
mémoire ; `lib/theme.ts` dérive tout le thème d'une seule couleur d'accent.

### Structure

- `src-tauri/src/auth/` — Microsoft → Xbox Live → XSTS → Minecraft Services, mode hors-ligne
- `src-tauri/src/instances/` — instances (dossiers isolés), mods, temps de jeu, renommage
- `src-tauri/src/minecraft/` — version, assets, bibliothèques, arguments de lancement
- `src-tauri/src/modloaders/` — Fabric, Quilt, Forge, NeoForge
- `src-tauri/src/providers/` — FTB, CurseForge
- `src-tauri/src/java/` — runtimes Java
- `src-tauri/src/download/` — téléchargements concurrents avec vérification sha1
- `src-tauri/src/launch/` — lancement, étapes, stats du processus, détection de crash
- `src/screens/` — écrans (Instances, Modpacks, Paramètres, Réglages d'instance, Mods, Lancement…)
- `src/components/shell/` — barre latérale, fond d'ambiance, barre d'activité, centre de notifications
- `src/components/instance/`, `launch/`, `console/`, `settings/` — composants par domaine
- `src/lib/` — logique pure testée (thème, RAM, logs, formatage, notifications, préréglages JVM)
- `src/store/` — état global (runtime, préférences, notifications)

> Ne nomme jamais un dossier source `logs` : le `.gitignore` l'ignorerait, et Tailwind ne
> scannerait pas ses classes.

### Publier une version

1. Mets la même version dans `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`
   (+ `Cargo.lock`) et `src-tauri/tauri.conf.json` — c'est elle que l'updater compare.
2. Tagge et pousse :

```bash
git tag -a v1.0.1 -m "v1.0.1"
git push origin main v1.0.1
```

Le tag déclenche `.github/workflows/release.yml` sur un runner Windows : build, release GitHub avec
les installeurs (`.exe`, `.msi`), `latest.json` signé pour l'auto-update, et une copie
`LargyLauncher-Setup.exe` au nom fixe pour le lien de téléchargement du README. Chaque push et chaque
pull request passent aussi par `.github/workflows/ci.yml` (tests, clippy, tsc, lint, format).

**Auto-update** : `tauri-plugin-updater` vérifie `releases/latest/download/latest.json`. Les
artefacts sont signés avec une clé privée (secret GitHub `TAURI_SIGNING_PRIVATE_KEY`, jamais dans le
dépôt) ; la clé publique est dans `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).

### Utiliser ta propre application Azure

L'application Azure du launcher est intégrée. Pour un fork, enregistre la tienne sur
[Entra ID](https://entra.microsoft.com) (App registrations → New registration, « Allow public client
flows » activé), fais-la valider pour l'API Minecraft ([aka.ms/mce-reviewappid](https://aka.ms/mce-reviewappid)),
puis mets son identifiant dans `azure_client_id` du fichier `settings.json` du launcher.
