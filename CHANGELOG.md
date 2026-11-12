# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
This file is generated from [Conventional Commits](https://www.conventionalcommits.org/)
with [git-cliff](https://git-cliff.org/).

## Unreleased

## 0.1.4 - 2026-11-12

### Added

- Show author and release notes in about dialog

### Fixed

- Include install files in release archive

### Internal

- Add GitHub Actions workflows for build, test, lint and release
- Automate release changelog and PKGBUILD bump
- Read the changelog version from the file

## 0.1.3 - 2026-11-07

### Fixed

- **store:** Add atomic compare-and-swap update for locations
- **store, script:** Atomic updates, proper GVariant, glib 0.10 compat
- **store:** Cache flatpak detection with OnceLock (avoids repeated flatpak list calls)

### Other

- Document AUR gweather-add-loc-bin package

## 0.1.2 - 2026-11-04

### Other

- Enhance with new dependencies and scripts
- Prepare 0.1.2: insert new locations at top, include LICENSE and script in release asset

## 0.1.1 - 2026-10-18

### Other

- Add screenshot
- Rework script
- Add Arch Linux PKGBUILD
- Document Arch Linux PKGBUILD

## 0.1.0 - 2026-10-17

### Changed

- Add Flatpak support and fix GVariant formatting

### Other

- Initial commit
- Update readme
- Add gweather-add-loc script
- Update
- Set executable permissions for script
- Add GTK4/libadwaita app in Rust
- Document GTK app
- Add GitHub Actions build workflow

