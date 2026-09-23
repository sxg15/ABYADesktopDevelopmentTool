---
name: game-archives
description: Discover, validate, and describe local ABYA archive folders for launch selection or transfer.
---

# Game Archives Skill

## Purpose

Provide one authoritative parser and filesystem boundary for local ABYA
archives used by launches and archive transfer.

## Ownership

Owns recursive launch discovery, fixed-save-directory transfer discovery,
`Main.PBArc` summary parsing, complete-folder enumeration, path containment,
symbolic-link rejection, and transfer source limits. It does not package,
transmit, install, or replace archives.

## Public Contracts

`ArchiveCatalogService`, `ArchiveOption`, `LevelOption`, and
`TransferableArchive`.

Launch discovery may recurse below an explicitly selected root. Transfer
discovery only inspects direct child folders of
`AppPaths.default_archive_root`. A manual source must be an existing file named
exactly `Main.PBArc`, directly inside the folder that will be transferred.

Transfer snapshots reject symbolic links, path escapes, more than 100,000
files, and more than 8 GiB of uncompressed content. The entire containing
folder is enumerated, not only `Main.PBArc`.

## Dependencies

Depends only on foundation paths, errors, JSON parsing, and filesystem APIs.
Instance and transfer modules consume its public DTOs and service.

## Validation

Test valid summaries, complete nested-folder enumeration, fixed-root scanning,
exact filename validation, malformed summaries, symbolic links, path escapes,
file-count limits, and byte limits.

## LLM Maintenance Rule

When changing archive parsing, discovery depth, source validation, limits, or
DTOs, update this Skill and the affected runtime/transfer contract documents in
the same change.
