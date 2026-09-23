# Game Logs Skill

## Purpose

Collect, persist, filter, and display structured logs from managed and
user-started external game instances.

## Ownership

Owns runtime log sessions, WebSocket log-batch ingestion, sequence replay,
deduplication, dropped-event accounting, pause/resume state, persistence, and
log queries.

## Public Contracts

`LogSession`, `RuntimeLogEvent`, `LogFilter`, log list/detail queries, and live
update events.

Every game connection automatically creates or resumes a log session. The
service persists each `log_batch` with insert-or-ignore deduplication before
the gateway sends `log_ack`. All events and the session sequence update in one
SQLite transaction, avoiding per-event WAL commits while preserving atomic
persistence-before-ack, and the service returns the latest persisted sequence
for replay. Each session retains only the latest 100,000 sequence positions;
older rows are pruned in the same transaction so free pages are reused by later
batches. Persisted messages are limited to 4,096 characters, identifiers and
metadata fields to 512 characters, and raw event JSON to 16,384 characters;
oversized raw payloads become a valid truncation marker. `logs_control` pauses
or resumes the connected game source. External
sources start streaming automatically and remain queryable after disconnect.

Stopped, failed, or interrupted log sessions may be deleted with all associated
events. Connecting, streaming, and reconnecting sessions must be stopped before
deletion.

Read `docs/ABYA_GAME_RUNTIME_CONTRACT.md` before changing SSE parsing, replay,
gap handling, or runtime log endpoint behavior.

## Dependencies

Depends on foundation, game-connections, and instance identity. It must not
launch or terminate processes.

## Validation

Test automatic session creation/resume, batch limits, transactional
persistence-before-ack, per-session retention pruning, field and raw-event
bounds, duplicate suppression, dropped counts, pause/resume, filtering,
pagination, disconnect history, active-session delete protection, and event
cascades.

## LLM Maintenance Rule

Storage repair closes active sessions and prunes each session to the bounded
retention window before compaction.

When changing event schemas, retention, filtering, connection behavior, or UI
contracts, update this Skill in the same change.
