# 🎮 ChomiamOS Dashboard (Édition Native Slint GUI)

> **Le tableau de bord système officiel de ChomiamOS** — Une véritable application de bureau native **100% Rust** développée avec le toolkit graphique moderne **Slint** et habillée selon la palette **Catppuccin Mocha**.

---

## ⚡ Pourquoi Slint ?

Contrairement aux tableaux de bord conventionnels basés sur des serveurs web ou Electron/Chromium :
- **Empreinte mémoire ultra-faible** : seulement **~20 à 30 Mo de RAM** au repos (contre 200 à 350 Mo pour une vue Web/Chromium). Idéal pour un système orienté Gaming où chaque mégaoctet de RAM compte !
- **Zéro latence & Démarrage instantané** : rendu direct accéléré matériellement via **Wayland** et **FemtoVG / OpenGL**.
- **Aucun serveur HTTP / Port requis** : l'interface communique directement en mémoire avec les structures et threads Rust.
- **Binaire unique autonome** : aucun runtime externe requis.

---

## ✨ Fonctionnalités Principales

### 1. 🖥️ Vue Système & Télémétrie
- **Télémétrie en direct (rafraîchissement 1.5s)** :
  - **CPU** : Détection du modèle (AMD Ryzen 7 7800X3D), fréquence en MHz en direct, jauge de charge globale et grille visuelle des **16 cœurs logiques** avec barres individuelles animées.
  - **GPU** : Détection de la carte graphique (AMD Radeon via sysfs `amdgpu` & `hwmon`), température thermique (°C), charge graphique et utilisation de la VRAM (utilisée / totale / %).
  - **Mémoire & Swap** : Utilisation de la mémoire vive et de l'espace Swap.
  - **Stockage** : Statut d'occupation des disques racine `/` et de la partition jeux `/mnt/Games`.
- **Mises à jour du Système** :
  - **Mettre à jour maintenant** (`nh os switch -u`) : Ouvre une console terminal intégrée avec streaming en direct des opérations Nix Flake.
  - **Mettre à jour au reboot** (`nh os boot -u`) : Permet de basculer la configuration sans perturber une session de jeu en cours.

### 2. 💽 Disques & Gestion des Générations
- **Surveillance de la volumétrie** du Nix Store.
- **Historique complet des générations NixOS** : Numéro de génération, date/heure, version du noyau Linux Zen, révision NixOS, avec badge **BOOTED** pour la génération actuellement en cours d'exécution.
- **Nettoyage intelligent** :
  - Conserver les **3 dernières générations** (`nh clean all --keep 3`).
  - Nettoyage total du Garbage Collector (`nh clean all`).
  - Optimisation des hardlinks du Nix Store (`nix-store --optimise`).

### 3. ⚙️ Configuration & Applications Optionnelles
- Modification directe et sécurisée du fichier `/etc/nixos/vars.nix` :
  - **Navigateur par défaut** (Chrome, Firefox, Brave, Zen Browser, Chromium).
  - **Discord** (Discord officiel, Vesktop avec plugins Vencord, Webcord).
  - **Écosystème Gaming** : Steam, Heroic, Lutris, Faugus Launcher, Decky Loader, GeForce NOW, Support Volants & Pédaliers.
  - **Centre d'Émulation** : Switch principal, Frontend ES-DE, RetroArch, Eden (Switch), Dolphin, PCSX2, PPSSPP, MelonDS, Azahar, mGBA, RPCS3.
  - **Multimédia & Utilitaires** : Stremio, VLC, MPV, Tailscale VPN, LocalSend, Motrix.
  - **Création & Outils Pro** : OBS Studio (plugins broadcast), Blender, Godot Engine, Kdenlive, Antigravity IDE, Pear Desktop, Virtualisation KVM, Suite IA locale.
- **Bouton « Enregistrer & Appliquer »** : Sauvegarde la configuration et lance automatiquement `nh os switch` dans la console intégrée !

---

## 🎨 Palette de Couleur : Catppuccin Mocha

L'interface utilise les codes chromatiques officiels du thème **Catppuccin Mocha** :
- **Base** : `#1e1e2e` (Fond principal)
- **Mantle** : `#181825` (Cartes et conteneurs)
- **Crust** : `#11111b` (Barre latérale et terminal de logs)
- **Surface 0 / 1 / 2** : `#313244` / `#45475a` / `#585b70` (Bordures et arrière-plans d'éléments)
- **Accents** :
  - **Mauve** : `#cba6f7`
  - **Blue** : `#89b4fa`
  - **Sapphire** : `#74c7ec`
  - **Green** : `#a6e3a1`
  - **Peach** : `#fab387`
  - **Teal** : `#94e2d5`
  - **Red** : `#f38ba8`

---

## 🚀 Démarrage & Utilisation

### Lancement direct
```bash
chomiamos-dashboard
```
Ou cliquez simplement sur l'icône **ChomiamOS Dashboard** dans votre lanceur d'applications GNOME.
