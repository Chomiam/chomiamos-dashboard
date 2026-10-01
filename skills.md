# 🧠 Base de Connaissances & Retours d'Expérience (Skills & REX) — ChomiamOS Dashboard

Ce document constitue la mémoire technique vivante du projet **ChomiamOS Dashboard** (`dashboard-chomiamos`). Il regroupe l'architecture interne, les mécanismes matériels Linux/NixOS, les leçons apprises (REX) et les pièges critiques à éviter.

---

## 🏛️ 1. Architecture Générale du Projet

- **Backend** : Rust (Tauri v2 + Tokio async runtime + Portable-PTY pour le terminal intégré).
- **Frontend** : Vanilla HTML5 / JavaScript (ESNext) / Vanilla CSS thématique Catppuccin Mocha.
- **Environnement de dev** : Nix Flake (`flake.nix`), exécuter les commandes Rust via `nix develop -c cargo ...`.
- **Règle n°1 de build** : **Zéro compilation locale lourde** (`nix build`, `cargo build --release` interdits). Seules les vérifications rapides `nix develop -c cargo check` et `cargo test --bin chomiamos-dashboard <filter>` sont autorisées en local.
- **Règle n°2 de versioning** : Chaque commit doit faire l'objet d'un bump de version (`Cargo.toml` et `tauri.conf.json`), être rédigé en français et poussé sur `origin main`.
- **Règle n°3 de résolution de bug** : Tout bug traité doit obligatoirement être tracé via une issue GitHub (`gh issue create`) puis référencé dans le commit (`closes #ID`).

---

## 💽 2. Gestion des Disques, Mise en Veille (Spindown) & Énergie

### A. Mécanisme de Persistance UDisks2 sous NixOS
- Contrairement aux distributions classiques qui utilisent des scripts `hdparm` au démarrage, UDisks2 est le gestionnaire de périphériques standard sous Linux/NixOS.
- **Emplacement de configuration** : `/etc/udisks2/<DriveId>.conf`.
  - Exemple de nom de fichier : `Samsung-SSD-990-EVO-Plus-2TB-S7U7NU0Y511611K.conf` ou `WDC-WD40EFRX-68N32N0-WD-WCC7K1234567.conf`.
  - Format INI strict :
    ```ini
    [ATA]
    StandbyTimeout=0
    APMLevel=255
    ```
- **Rechargement automatique** : Le démon `udisksd` surveille `/etc/udisks2/` via `inotify` et applique immédiatement les paramètres au firmware du disque lors de l'écriture ou de la suppression du fichier.
- **Appel D-Bus direct** :
  ```bash
  gdbus call --system --dest org.freedesktop.UDisks2 \
    --object-path /org/freedesktop/UDisks2/drives/<DriveObject> \
    --method org.freedesktop.UDisks2.Drive.SetConfiguration \
    "{'ata-pm-standby': <0>, 'ata-apm-level': <255>}" "{}"
  ```

### B. Tables de Correspondance des Valeurs ATA
1. **`StandbyTimeout` (Spindown)** :
   - `0` : **Désactivé** (le disque ne se met JAMAIS en veille automatique, rotation continue).
   - `1..=240` : Multiples de 5 secondes (`valeur * 5s`). Exemple : `60` = 5 min, `120` = 10 min, `240` = 20 min.
   - `241..=251` : Multiples de 30 minutes (`(valeur - 240) * 30 min`). Exemple : `241` = 30 min, `242` = 1 heure, `244` = 2 heures.
   - `252` : 21 minutes.
   - `253` : Délai constructeur (8 à 12 heures).
   - `255` : 21 minutes et 15 secondes.
2. **`APMLevel` (Advanced Power Management)** :
   - `255` : **Désactivé / Performance Maximale**. Élimine le parcage agressif des têtes de lecture et les micro-gels d'accès.
   - `254` : Performance maximale sans mise en veille.
   - `128` : Équilibré (pas de mise en veille mécanique forcée).
   - `1..=127` : Économique avec autorisation de mise en veille mécanique.

### C. Runtime Power Management Noyau Linux (sysfs)
- Fichier : `/sys/block/<dev>/device/power/control` ou `/sys/block/<dev>/power/control`.
  - `on` : Le noyau interdit la suspension automatique du contrôleur de bus PCIe / SATA / USB.
  - `auto` : Le noyau autorise la suspension d'énergie à chaud.
- Pour désactiver totalement la veille : toujours combiner `StandbyTimeout=0`, `APMLevel=255` et `power/control = "on"`.

### D. Télémétrie Matérielle & Sondes Thermiques
- **Type de support** :
  - Si `/sys/block/<dev>/queue/rotational` vaut `1` : **Disque mécanique HDD à plateaux**.
  - Si `0` et préfixe `nvme` : **SSD NVMe M.2**.
  - Si `0` et préfixe `sd` : **SSD SATA**.
- **Température S.M.A.R.T.** :
  - UDisks2 expose `SmartTemperature` en degrés Kelvin (ex: `326`).
  - Conversion Celsius : `temp_c = kelvin - 273` (ex: `326 - 273 = 53°C`).

---

## 🔗 3. Montages Permanents & Immunité GitHub (`mount.nix`)

- Fichier cible : `/etc/nixos/hosts/desktop/mount.nix`.
- Chaque montage de disque généré par le Dashboard injecte obligatoirement :
  - `defaults`
  - `nofail` (empêche le blocage au boot si le disque est débranché ou indisponible)
  - `x-systemd.device-timeout=5s` & `x-systemd.mount-timeout=5s` (limite le timeout d'attente à 5s au démarrage)
  - `x-gvfs-show` (assure l'affichage natif dans l'explorateur de fichiers GNOME / Nautilus / Thunar)
  - Permissions tmpfiles utilisateur : `"d <mount_point> 0775 ${username} users -"`
- **Détection des orphelins** : Si un disque est débranché ou reformaté avec un nouvel UUID, `check_missing_persistent_mounts` identifie les déclarations manquantes et propose un nettoyage sécurisé en 1 clic.

---

## 🔒 4. Élévation de Privilèges sous NixOS (`pkexec`)

- Toutes les écritures système (`/etc/nixos/`, `/etc/udisks2/`, montage `mount`, formatage `mkfs`) s'exécutent via `pkexec`.
- **Pattern éprouvé** :
  ```rust
  let script = format!("cat << 'EOF' > {}\n{}\nEOF", path, content);
  let output = Command::new("pkexec")
      .args(["sh", "-c", &script])
      .output()?;
  ```
- Les scripts doivent être compacts, auto-suffisants, et gérer proprement `2>/dev/null || true` sur les sous-commandes secondaires.

---

## 🌐 5. Réseau, Pare-feu, DNS & WireGuard

- **Conflits sur le port 53 (DNS)** :
  - `systemd-resolved` écoute par défaut sur `127.0.0.53:53`.
  - Pour libérer le port 53 (AdGuard Home, Pi-hole) sous NixOS, désactiver le stub listener :
    ```nix
    services.resolved.settings.Resolve.DNSStubListener = "no";
    ```
- **Sondes Réseau Non-Bloquantes** :
  - Ne jamais exécuter de ping ou de requête réseau synchrone bloquante dans le thread principal de l'API.
  - Toujours utiliser `tokio::spawn` avec des timeouts stricts (`tokio::time::timeout(Duration::from_millis(350), ...)`).
- **Diagnostics WireGuard & sFTP** :
  - Classifier clairement l'origine des erreurs de connexion (Côté PC local, Box/Routeur, ou Serveur/Ami distant).
  - Filtrer les utilisateurs système (`/run/current-system/sw/bin/nologin`) des accès SSH/sFTP interactifs.

---

## 🎨 6. Résilience Frontend & Design System Catppuccin Mocha

- **Pattern SWR (Stale-While-Revalidate)** :
  - Hydrater immédiatement l'interface à 0ms depuis les structures ou le cache local, puis déclencher l'appel réseau en arrière-plan.
- **`Promise.allSettled` systématique** :
  - Ne jamais utiliser `Promise.all` sur les rafraîchissements composites (`loadStorageDevices`, `refreshAll`). Une erreur isolée ne doit jamais faire planter l'ensemble de la page.
- **Toasts & Modales** :
  - Les toasts d'erreurs système doivent persister au moins 10 à 12 secondes et offrir un bouton de copie d'erreur.
  - `#toast-container` doit être au premier plan absolu (`z-index: 100050 !important;` avec `pointer-events: none;`).
- **Boutons Disabled Passifs Proscrits** :
  - Ne jamais laisser un bouton muet sans retour visuel au clic. Préférer un bouton cliquable qui affiche un toast explicatif et guide l'utilisateur vers le champ requis.

---

## 📋 7. Journal des Évolutions Majeures

| Version | Date | Description de l'évolution & Apprentissages |
| :--- | :--- | :--- |
| **`v0.5.10`** | 01/10/2026 | **Suite de Gestion d'Énergie & Anti-Veille des Disques** : Intégration UDisks2 (`StandbyTimeout=0`, `APMLevel=255`), forçage du Runtime PM noyau Linux (`/sys/block/.../power/control`), télémétrie S.M.A.R.T. thermique et badges visuels Catppuccin Mocha. Ajout des règles d'agent `agent.md` et de la mémoire `skills.md`. |
| **`v0.5.9`** | 01/10/2026 | Optimisation WireGuard, fiabilisation des endpoints et amélioration des partages sFTP. |
