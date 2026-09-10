# 🎮 ChomiamOS Dashboard

> **Le tableau de bord officiel de ChomiamOS** — Alliant la puissance et la réactivité d'un backend en **Rust** à une interface moderne et élégante conçue selon la palette **Catppuccin Mocha**.

---

## ✨ Fonctionnalités Principales

### 1. 🖥️ Vue Système & Mises à jour
- **Télémétrie en temps réel** (rafraîchissement toutes les 1.5 secondes) :
  - **CPU** : Modèle (AMD Ryzen 7 7800X3D), fréquence temps réel (MHz), jauge circulaire d'utilisation globale, et barres de progression dynamiques pour l'intégralité des cœurs/threads.
  - **GPU** : Détection automatique AMD Radeon (via sysfs `drm/card*/device` & `hwmon`), température thermique, jauge d'activité graphique et jauge dédiée à l'utilisation de la VRAM (Mo/Go).
  - **Mémoire & Swap** : Utilisation de la RAM (Go / % utilisé) et de l'espace Swap.
  - **Stockage Principal** : Statut d'occupation des disques racine `/` et partition de jeux `/mnt/Games`.
- **Gestion des Mises à jour Système** :
  - Détection de l'état des canaux et dépôts NixOS.
  - **Mise à jour Immédiate** : Déclenche `nh os switch -u` dans une console en streaming en direct.
  - **Mise à jour au Redémarrage** : Déclenche `nh os boot -u` (permettant de continuer à jouer ou travailler sans interruption et de basculer la configuration au prochain démarrage).

### 2. 💽 Disques & Gestion des Générations
- **Surveillance du Nix Store** et volume de stockage occupé par le système.
- **Historique complet des générations NixOS** : Numéro de génération, date/heure de déploiement, version du noyau Linux Zen, révision NixOS, et mise en évidence de la génération active (`Booted`).
- **Nettoyage intelligent des anciennes générations** :
  - Nettoyer et conserver les **3 dernières générations** (`nh clean all --keep 3`).
  - Nettoyage total du ramasse-miettes (`nh clean all`).
  - Optimisation des hardlinks du Store (`nix-store --optimise`).

### 3. ⚙️ Configuration & Applications Optionnelles
- Modification directe et sécurisée du fichier `/etc/nixos/vars.nix` :
  - **Navigateur Web par défaut** (Google Chrome, Firefox, Brave, Zen Browser, Chromium).
  - **Client Discord** (Discord officiel, Vesktop avec plugins Vencord préactivés, Webcord).
  - **Écosystème Gaming** : Steam, Lutris, Heroic Games Launcher, Faugus Launcher, Decky Loader, GeForce NOW, Support Volants & Pédaliers FF.
  - **Centre d'Émulation** : Switch d'activation global, Frontend (ES-DE EmulationStation), RetroArch, Eden (Yuzu/Switch), Dolphin, PCSX2, PPSSPP, MelonDS, Azahar (Citra), mGBA, RPCS3 (PS3).
  - **Multimédia & Utilitaires** : Stremio, VLC, MPV, Tailscale VPN, LocalSend, Motrix Download Manager.
  - **Création & Outils Pro** : DaVinci Resolve (Studio/Standard/None), Blender, Godot Engine, Kdenlive, OBS Studio (avec plugins broadcast), Antigravity IDE, Pear Desktop, Virtualisation KVM/QEMU, Suite IA locale (Ollama & WebUI).
- **Bouton « Enregistrer & Appliquer »** : Réécrit le fichier Nix et exécute automatiquement `nh os switch` en affichant la sortie ANSI en direct !

---

## 🎨 Palette de Couleur : Catppuccin Mocha

L'interface utilise les codes chromatiques officiels du thème **Catppuccin Mocha** :
- **Base** : `#1e1e2e` (Fond principal)
- **Mantle** : `#181825` (Barres latérales et conteneurs)
- **Crust** : `#11111b` (Fond de console terminal)
- **Surface 0 / 1 / 2** : `#313244` / `#45475a` / `#585b70` (Cartes et bordures)
- **Accents** :
  - **Mauve** : `#cba6f7`
  - **Blue** : `#89b4fa`
  - **Sapphire** : `#74c7ec`
  - **Green** : `#a6e3a1`
  - **Peach** : `#fab387`
  - **Red** : `#f38ba8`

---

## 🚀 Démarrage & Utilisation

### En mode Développeur
```bash
cd /home/chomiam/Projects/dashboard-chomiamos
cargo run
```

### En mode Application Autonome
```bash
/home/chomiam/Projects/dashboard-chomiamos/target/release/chomiamos-dashboard
```
Par défaut, le dashboard démarre le serveur web sur `http://127.0.0.1:9090` et ouvre automatiquement la fenêtre de l'application via le mode fenêtre autonome de Chrome (`--app=http://127.0.0.1:9090`) ou le navigateur par défaut.

Pour désactiver l'ouverture automatique du navigateur (mode headless/service) :
```bash
./target/release/chomiamos-dashboard --no-open
```
