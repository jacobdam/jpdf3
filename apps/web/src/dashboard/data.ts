// The metrics snapshot the dashboard renders (specs/tooling/dashboard.md, "Snapshot fields").
// `cargo xtask dashboard` writes it; the page treats it as untrusted input and accepts only
// version 1 with every field of the right type. Anything else is "missing or invalid".

export type Week = {
  start: string;
  end: string;
  mergedPrs: number;
  medianRounds: number | null;
  sessions: number | null;
};

export type Milestone = {
  title: string;
  open: number;
  closed: number;
  closed28d: number;
  forecast: string | null;
};

export type Snapshot = {
  generated: string;
  until: string;
  weeks: Week[];
  hasIndex: boolean;
  journal: {
    prs: number;
    median: number | null;
    max: number | null;
    histogram: { entries: number; prs: number }[];
  };
  blocked: { number: number; title: string; ageDays: number }[];
  milestones: Milestone[];
  truncated: string[];
  malformedIndexRows: number;
};

/** Thrown for the first field that doesn't have its type. */
class Invalid extends Error {}

type Obj = Record<string, unknown>;

function obj(v: unknown): Obj {
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new Invalid("object");
  return v as Obj;
}

function list(v: unknown): unknown[] {
  if (!Array.isArray(v)) throw new Invalid("array");
  return v;
}

/** A non-negative safe integer (a count). */
function count(v: unknown): number {
  if (typeof v !== "number" || !Number.isSafeInteger(v) || v < 0) throw new Invalid("count");
  return v;
}

function countOrNull(v: unknown): number | null {
  return v === null ? null : count(v);
}

function text(v: unknown): string {
  if (typeof v !== "string") throw new Invalid("string");
  return v;
}

/** `YYYY-MM-DD` that is a real calendar date. */
function day(v: unknown): string {
  const s = text(v);
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (m === null) throw new Invalid("date");
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const t = new Date(Date.UTC(y, mo - 1, d));
  if (t.getUTCFullYear() !== y || t.getUTCMonth() !== mo - 1 || t.getUTCDate() !== d)
    throw new Invalid("date");
  return s;
}

function dayOrNull(v: unknown): string | null {
  return v === null ? null : day(v);
}

/** The snapshot, or null when `raw` isn't a valid version 1 snapshot. */
export function parseSnapshot(raw: unknown): Snapshot | null {
  try {
    const o = obj(raw);
    if (o.version !== 1) return null;
    const sessions = text(o.sessions);
    if (sessions !== "index" && sessions !== "missing") return null;
    const j = obj(o.journal_entries);
    return {
      generated: day(o.generated),
      until: day(o.until),
      weeks: list(o.weeks).map((w) => {
        const x = obj(w);
        return {
          start: day(x.start),
          end: day(x.end),
          mergedPrs: count(x.merged_prs),
          medianRounds: countOrNull(x.median_rounds),
          sessions: countOrNull(x.sessions),
        };
      }),
      hasIndex: sessions === "index",
      journal: {
        prs: count(j.prs),
        median: countOrNull(j.median),
        max: countOrNull(j.max),
        histogram: list(j.histogram).map((h) => {
          const x = obj(h);
          return { entries: count(x.entries), prs: count(x.prs) };
        }),
      },
      blocked: list(o.blocked).map((b) => {
        const x = obj(b);
        return { number: count(x.number), title: text(x.title), ageDays: count(x.age_days) };
      }),
      milestones: list(o.milestones).map((m) => {
        const x = obj(m);
        return {
          title: text(x.title),
          open: count(x.open),
          closed: count(x.closed),
          closed28d: count(x.closed_28d),
          forecast: dayOrNull(x.forecast),
        };
      }),
      truncated: list(o.truncated).map(text),
      malformedIndexRows: count(o.malformed_index_rows),
    };
  } catch (e) {
    if (e instanceof Invalid) return null;
    throw e;
  }
}
