# 1. Record architecture decisions

Status: accepted

## Context
The project is built by autonomous agent sessions with no shared memory. Decisions must
survive between sessions, and their reasons must be visible to the owner.

## Decision
Every non-trivial decision is recorded as `docs/adr/NNNN-short-title.md` with Status, Context,
Decision and Consequences. Owner instructions (`human` issues) override ADRs. Spec ambiguities
are decided in this order: ISO 32000-2 text, errata / PDF Association notes, reference-engine
consensus, then the safer behavior.

## Consequences
Sessions read relevant ADRs before changing an area. Superseded ADRs are marked, never deleted.
