// The judge report the deployed site holds at judge/report/report.json (the smoke tier of the
// newest push to main; specs/tooling/dashboard.md, Page). Untrusted input: only the fields
// below are read, each checked; anything else is "invalid".

import { count, Invalid, obj, rateOrNull, text } from "./data";

/** Longest report text parsed (UTF-16 code units). */
export const MAX_REPORT_CHARS = 4 * 1024 * 1024;

export type JourneyCount = { title: string; passing: number; journeys: number };

export type JudgeReport = {
  openRate: number | null;
  renderMatch: number | null;
  textF1: number | null;
  crashes: number;
  /** Journeys that aren't superseded, per milestone: `M<n>` by number, then other titles. */
  milestones: JourneyCount[];
};

export type ReportState =
  | { state: "loading" }
  | { state: "missing" }
  | { state: "invalid" }
  | { state: "ok"; report: JudgeReport };

function boolean(v: unknown): boolean {
  if (typeof v !== "boolean") throw new Invalid("boolean");
  return v;
}

/** `M<n>` first by n, then the rest in code-point order. */
function byMilestone(a: string, b: string): number {
  const n = (t: string) => (/^M\d+$/.test(t) ? Number(t.slice(1)) : null);
  const [x, y] = [n(a), n(b)];
  if (x !== null && y !== null) return x - y || (a < b ? -1 : a > b ? 1 : 0);
  if (x !== null) return -1;
  if (y !== null) return 1;
  return a < b ? -1 : a > b ? 1 : 0;
}

/** The report, or null when `body` isn't a smoke judge report. */
export function parseReport(body: string): JudgeReport | null {
  if (body.length > MAX_REPORT_CHARS) return null;
  try {
    const o = obj(JSON.parse(body) as unknown);
    if (o.tier !== "smoke") return null;
    const m = obj(o.metrics);
    const counts = new Map<string, JourneyCount>();
    for (const j of Object.values(obj(o.journeys))) {
      const x = obj(j);
      const title = text(x.milestone);
      // verdict.py writes null ("unknown") when a platform had no e2e results in the run;
      // like its journeys_passing, only true counts as passing.
      const pass = x.pass === null ? false : boolean(x.pass);
      if (boolean(x.superseded)) continue;
      const c = counts.get(title) ?? { title, passing: 0, journeys: 0 };
      c.journeys += 1;
      if (pass) c.passing += 1;
      counts.set(title, c);
    }
    return {
      openRate: rateOrNull(m.open_rate),
      renderMatch: rateOrNull(m.render_match),
      textF1: rateOrNull(m.text_f1),
      crashes: count(m.crashes),
      milestones: [...counts.values()].sort((a, b) => byMilestone(a.title, b.title)),
    };
  } catch (e) {
    if (e instanceof Invalid || e instanceof SyntaxError) return null;
    throw e;
  }
}
