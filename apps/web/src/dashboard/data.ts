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

/** One week of the judge history (nightly smoke metrics); every value null without a report. */
export type JudgeWeek = {
  start: string;
  run: number | null;
  day: string | null;
  openRate: number | null;
  renderMatch: number | null;
  textF1: number | null;
  crashes: number | null;
  journeysPassing: number | null;
  journeys: number | null;
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
  /** null: no judge history (no `judge` field, or source `missing`). */
  judge: JudgeWeek[] | null;
  truncated: string[];
  malformedIndexRows: number;
};

/** Thrown for the first field that doesn't have its type. */
export class Invalid extends Error {}

type Obj = Record<string, unknown>;

export function obj(v: unknown): Obj {
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new Invalid("object");
  return v as Obj;
}

export function list(v: unknown): unknown[] {
  if (!Array.isArray(v)) throw new Invalid("array");
  return v;
}

/** A non-negative safe integer (a count). */
export function count(v: unknown): number {
  if (typeof v !== "number" || !Number.isSafeInteger(v) || v < 0) throw new Invalid("count");
  return v;
}

function countOrNull(v: unknown): number | null {
  return v === null ? null : count(v);
}

export function text(v: unknown): string {
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

/** A number from 0 to 1, or null. */
export function rateOrNull(v: unknown): number | null {
  if (v === null) return null;
  if (typeof v !== "number" || !Number.isFinite(v) || v < 0 || v > 1) throw new Invalid("rate");
  return v;
}

/** The `judge` field: null when absent or `source` is `missing`; `starts` are `weeks[i].start`. */
function judgeWeeks(v: unknown, starts: string[]): JudgeWeek[] | null {
  if (v === undefined) return null; // a snapshot written before #306
  const o = obj(v);
  const source = text(o.source);
  if (source !== "nightly" && source !== "missing") throw new Invalid("judge source");
  const weeks = list(o.weeks).map((w) => {
    const x = obj(w);
    return {
      start: day(x.start),
      run: countOrNull(x.run),
      day: dayOrNull(x.day),
      openRate: rateOrNull(x.open_rate),
      renderMatch: rateOrNull(x.render_match),
      textF1: rateOrNull(x.text_f1),
      crashes: countOrNull(x.crashes),
      journeysPassing: countOrNull(x.journeys_passing),
      journeys: countOrNull(x.journeys),
    };
  });
  if (weeks.length !== starts.length || weeks.some((w, i) => w.start !== starts[i]))
    throw new Invalid("judge weeks");
  return source === "missing" ? null : weeks;
}

/** The snapshot, or null when `raw` isn't a valid version 1 snapshot. */
export function parseSnapshot(raw: unknown): Snapshot | null {
  try {
    const o = obj(raw);
    if (o.version !== 1) return null;
    const sessions = text(o.sessions);
    if (sessions !== "index" && sessions !== "missing") return null;
    const j = obj(o.journal_entries);
    const weeks = list(o.weeks);
    // The writer emits twelve consecutive weeks and four histogram objects, entries 1..4 in order
    // (specs/tooling/dashboard.md); the charts key their bars by those labels.
    if (weeks.length !== 12) return null;
    const histogram = list(j.histogram).map((h) => {
      const x = obj(h);
      return { entries: count(x.entries), prs: count(x.prs) };
    });
    if (histogram.length !== 4 || histogram.some((h, i) => h.entries !== i + 1)) return null;
    const parsed = {
      generated: day(o.generated),
      until: day(o.until),
      weeks: weeks.map((w) => {
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
        histogram,
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
      judge: null as JudgeWeek[] | null,
      truncated: list(o.truncated).map(text),
      malformedIndexRows: count(o.malformed_index_rows),
    };
    parsed.judge = judgeWeeks(
      o.judge,
      parsed.weeks.map((w) => w.start),
    );
    const starts = new Set(parsed.weeks.map((w) => w.start));
    if (starts.size !== parsed.weeks.length) return null;
    return parsed;
  } catch (e) {
    if (e instanceof Invalid) return null;
    throw e;
  }
}
