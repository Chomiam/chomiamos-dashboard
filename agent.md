# 🤖 Directives & Règles pour les Agents IA — ChomiamOS Dashboard

Ce document définit les règles opérationnelles, les flux de travail et les standards stricts que tout agent IA doit **impérativement et sans exception** respecter lorsqu'il intervient sur le projet `dashboard-chomiamos`.

---

## 📌 1. Règle d'Or : Versioning, Commits & GitHub Push Systématiques

> **Chaque modification apportée au projet DOIT être commitée, poussée sur GitHub et faire l'objet d'une montée de version.**

1. **Montée de version systématique (Bump Version)** :
   - À chaque ajout de fonctionnalité, refactorisation ou correction, incrémenter la version selon le Semantic Versioning (`MAJOR.MINOR.PATCH`) :
     - `Cargo.toml` (`[package] version = "X.Y.Z"`)
     - `tauri.conf.json` (`"version": "X.Y.Z"`)
   - Ne jamais laisser de modifications non versionnées.

2. **Commits détaillés et obligatoirement en français** :
   - Format conventionnel : `<type>(<scope>): <description claire et informative en français>`
   - Types acceptés : `feat`, `fix`, `chore`, `refactor`, `style`, `docs`, `test`.
   - Le message doit expliquer *ce qui a été fait* et *pourquoi*, de manière exhaustive et soignée.

3. **Push automatique vers GitHub (`origin main`)** :
   - Tout commit validé doit être immédiatement poussé sur le dépôt distant :
     ```bash
     git push origin main
     ```

---

## 🐛 2. Gestion des Bugs : Création Obligatoire d'une Issue GitHub

> **Tout travail de résolution d'un bug ou dysfonctionnement DOIT être accompagné d'une Issue GitHub dédiée.**

1. **Création préalable ou conjointe de l'issue** :
   - Utiliser GitHub CLI (`gh issue create`) :
     ```bash
     gh issue create --title "fix(<scope>): titre du bug" --body "### Description du problème\n...\n### Reproduction\n...\n### Solution apportée\n..." --label "bug"
     ```
2. **Liaison explicite dans le commit** :
   - Associer le commit de résolution à l'issue pour fermeture automatique :
     ```bash
     git commit -m "fix(<scope>): résolution du bug X (closes #<issue_number>)"
     ```
3. **Documentation du diagnostic** :
   - Noter la cause racine et la solution dans l'issue et dans `skills.md`.

---

## 🧠 3. Documentation Continue dans `skills.md`

> **L'agent doit consigner tout apprentissage, découverte technique, piège rencontré et bonne pratique dans le fichier `skills.md` et l'actualiser régulièrement.**

- Chaque nouveau mécanisme compris (ex: ioctl ATA, UDisks2 D-Bus, sysfs Linux, particularités NixOS, gestion des sockets IPC) doit être documenté dans [skills.md](file:///home/chomiam/Projects/dashboard-chomiamos/skills.md).
- Ce fichier sert de mémoire persistante et de base de connaissances (REX) partagée pour toutes les sessions futures.

---

## 🛡️ 4. Règles Système NixOS & Runtimes

1. **Compatibilité NixOS & `nix-ld` avec discernement** :
   - Toujours évaluer de manière critique si un binaire tiers ou outil a réellement besoin de `nix-ld` avant tout ajout.
2. **Zéro compilation locale lourde** :
   - **Interdiction formelle** de lancer des compilations locales lourdes (`nix build`, recompilation d'ISO, `cargo build --release`).
   - Pour la vérification syntaxique et de types, s'en tenir strictement à `nix develop -c cargo check` ou des tests unitaires ciblés (`cargo test --bin chomiamos-dashboard <test_name>`).
   - Les builds de release sont délégués aux workflows GitHub Actions distants et au cache binaire Cachix.
3. **Élévation de privilèges via `pkexec`** :
   - Pour toute action système nécessitant les privilèges root (écriture dans `/etc/udisks2/`, `/etc/nixos/hosts/desktop/mount.nix`, etc.), utiliser `pkexec` avec des scripts idempotents, sécurisés et validés.
4. **Appels asynchrones non-bloquants (Tokio)** :
   - Tout appel CLI ou I/O disque lourd côté Rust doit être encapsulé dans `tokio::task::spawn_blocking` pour ne jamais figer la boucle d'événements du serveur ou de Tauri.
5. **Pattern SWR & UI Haute Performance** :
   - Rendre immédiatement l'UI depuis le cache ou les états locaux (0ms).
   - Toujours utiliser `Promise.allSettled` plutôt que `Promise.all` pour éviter qu'un échec partiel ne bloque tout le tableau de bord.
   - Respecter le design system Catppuccin Mocha.
