import Alert from "@mui/material/Alert";
import Box from "@mui/material/Box";
import Stack from "@mui/material/Stack";
import Table from "@mui/material/Table";
import TableBody from "@mui/material/TableBody";
import TableCell from "@mui/material/TableCell";
import TableHead from "@mui/material/TableHead";
import TableRow from "@mui/material/TableRow";
import Typography from "@mui/material/Typography";
import type { ReactNode } from "react";
import { type Bar, BarChart } from "./BarChart";
import type { JudgeWeek, Milestone, Snapshot, Week } from "./data";
import type { JudgeReport, ReportState } from "./judge";

// specs/tooling/dashboard.md, Page. Snapshot text (issue titles) is rendered as React text.

export const INVALID = "No metrics snapshot: data.json is missing or invalid.";
export const NO_INDEX = "Missing: the snapshot has no session index.";
export const NO_JUDGE_HISTORY = "Missing: the snapshot has no judge history.";
export const NO_REPORT = "Missing: this site has no judge report.";
export const BAD_REPORT = "Invalid: the judge report doesn't have the expected format.";

function weekLabel(w: Week): string {
  // The last week is partial when --until isn't its Sunday.
  const sunday = new Date(`${w.start}T00:00:00Z`);
  sunday.setUTCDate(sunday.getUTCDate() + 6);
  const full = sunday.toISOString().slice(0, 10) === w.end;
  return full ? `Week of ${w.start}` : `Week of ${w.start} (to ${w.end})`;
}

function forecastText(m: Milestone): string {
  if (m.open === 0) return "done";
  if (m.forecast !== null) return m.forecast;
  if (m.closed28d === 0) return "no closures in 4 weeks";
  return "missing";
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Box component="section" aria-label={title}>
      <Typography variant="h6" component="h2" sx={{ mb: 1 }}>
        {title}
      </Typography>
      {children}
    </Box>
  );
}

export function Dashboard({
  snapshot,
  report,
}: {
  snapshot: Snapshot | null | undefined;
  report: ReportState;
}) {
  return (
    <Box component="main" sx={{ maxWidth: 960, mx: "auto", my: 4, px: 3 }}>
      <Stack spacing={4}>
        <Typography variant="h6" component="h1">
          jpdf metrics
        </Typography>
        {snapshot === undefined ? (
          <Typography>Loading metrics…</Typography>
        ) : snapshot === null ? (
          <Alert severity="warning">{INVALID}</Alert>
        ) : (
          <Sections s={snapshot} report={report} />
        )}
      </Stack>
    </Box>
  );
}

function Sections({ s, report }: { s: Snapshot; report: ReportState }) {
  const weeks = (f: (w: Week) => number | null): Bar[] =>
    s.weeks.map((w) => ({ label: weekLabel(w), value: f(w) }));
  return (
    <>
      <Typography variant="body2" color="text.secondary">
        Snapshot generated {s.generated}, data up to {s.until}. Refreshed by each retro (cargo xtask
        dashboard).
      </Typography>
      {s.truncated.length > 0 && (
        <Alert severity="warning">
          These lists hit the 1000-entry limit and may be cut off: {s.truncated.join(", ")}.
        </Alert>
      )}
      <Section title="Merged PRs per week">
        <BarChart
          title="Merged PRs per week"
          column="Week"
          valueColumn="Merged PRs"
          bars={weeks((w) => w.mergedPrs)}
        />
      </Section>
      <Section title="Median review rounds per week">
        <BarChart
          title="Median review rounds per week"
          column="Week"
          valueColumn="Median rounds"
          bars={weeks((w) => w.medianRounds)}
        />
      </Section>
      <Section title="Sessions per week">
        {s.hasIndex ? (
          <>
            <BarChart
              title="Sessions per week"
              column="Week"
              valueColumn="Sessions"
              bars={weeks((w) => w.sessions)}
            />
            {s.malformedIndexRows > 0 && (
              <Typography variant="body2" color="text.secondary">
                {s.malformedIndexRows} malformed session index rows were skipped.
              </Typography>
            )}
          </>
        ) : (
          <Typography>{NO_INDEX}</Typography>
        )}
      </Section>
      <Section title="Sessions per merged PR">
        <Typography variant="body2" sx={{ mb: 1 }}>
          Session journal entries per merged PR: {s.journal.prs} PRs, median{" "}
          {s.journal.median ?? "missing"}, max {s.journal.max ?? "missing"}.
        </Typography>
        <BarChart
          title="Merged PRs by session journal entries"
          column="Journal entries"
          valueColumn="Merged PRs"
          bars={s.journal.histogram.map((h) => ({
            label: h.entries >= 4 ? "4 or more" : String(h.entries),
            value: h.prs,
          }))}
        />
      </Section>
      <Section title="Blocked issues">
        {s.blocked.length === 0 ? (
          <Typography>No blocked issues.</Typography>
        ) : (
          <Table size="small" aria-label="Blocked issues">
            <TableHead>
              <TableRow>
                <TableCell component="th">Issue</TableCell>
                <TableCell component="th">Title</TableCell>
                <TableCell component="th" align="right">
                  Age (days)
                </TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {s.blocked.map((b) => (
                <TableRow key={b.number}>
                  <TableCell>#{b.number}</TableCell>
                  <TableCell>{b.title}</TableCell>
                  <TableCell align="right">{b.ageDays}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </Section>
      <Section title="Milestone forecast">
        <Typography variant="body2" sx={{ mb: 1 }}>
          Finish date at the trailing 4-week closing rate: open issues × 28 days ÷ issues closed in
          the last 28 days.
        </Typography>
        <Table size="small" aria-label="Milestone forecast">
          <TableHead>
            <TableRow>
              <TableCell component="th">Milestone</TableCell>
              <TableCell component="th" align="right">
                Closed
              </TableCell>
              <TableCell component="th" align="right">
                Open
              </TableCell>
              <TableCell component="th" align="right">
                Closed in the last 4 weeks
              </TableCell>
              <TableCell component="th">Forecast finish</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {s.milestones.map((m) => (
              <TableRow key={m.title}>
                <TableCell>{m.title}</TableCell>
                <TableCell align="right">{m.closed}</TableCell>
                <TableCell align="right">{m.open}</TableCell>
                <TableCell align="right">{m.closed28d}</TableCell>
                <TableCell>{forecastText(m)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Section>
      <Section title="Judge metrics, latest main run">
        {report.state === "loading" ? (
          <Typography>Loading the judge report…</Typography>
        ) : report.state === "missing" ? (
          <Typography>{NO_REPORT}</Typography>
        ) : report.state === "invalid" ? (
          <Alert severity="warning">{BAD_REPORT}</Alert>
        ) : (
          <LatestReport r={report.report} />
        )}
      </Section>
      <Section title="Judge metrics per week">
        {s.judge === null ? (
          <Typography>{NO_JUDGE_HISTORY}</Typography>
        ) : (
          <JudgeHistory weeks={s.weeks} judge={s.judge} />
        )}
      </Section>
    </>
  );
}

function missing(v: number | null): string {
  return v === null ? "missing" : String(v);
}

function LatestReport({ r }: { r: JudgeReport }) {
  return (
    <Stack spacing={2}>
      <Typography variant="body2" color="text.secondary">
        The smoke corpus report of the newest push to main (judge/report/report.json).
      </Typography>
      <Table size="small" aria-label="Smoke corpus, latest run" sx={{ maxWidth: 600 }}>
        <TableHead>
          <TableRow>
            <TableCell component="th">Metric</TableCell>
            <TableCell component="th" align="right">
              Value
            </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {(
            [
              ["Open rate", missing(r.openRate)],
              ["Render match", missing(r.renderMatch)],
              ["Text F1", missing(r.textF1)],
              ["Crashes", String(r.crashes)],
            ] as const
          ).map(([name, value]) => (
            <TableRow key={name}>
              <TableCell>{name}</TableCell>
              <TableCell align="right">{value}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      {r.milestones.length === 0 ? (
        <Typography>No journeys in the report.</Typography>
      ) : (
        <Table size="small" aria-label="Journeys passing per milestone" sx={{ maxWidth: 600 }}>
          <TableHead>
            <TableRow>
              <TableCell component="th">Milestone</TableCell>
              <TableCell component="th" align="right">
                Passing
              </TableCell>
              <TableCell component="th" align="right">
                Journeys
              </TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {r.milestones.map((m) => (
              <TableRow key={m.title}>
                <TableCell>{m.title}</TableCell>
                <TableCell align="right">{m.passing}</TableCell>
                <TableCell align="right">{m.journeys}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </Stack>
  );
}

function JudgeHistory({ weeks, judge }: { weeks: Week[]; judge: JudgeWeek[] }) {
  // The validator pins judge[i].start to weeks[i].start, so both share the week labels.
  const bars = (f: (j: JudgeWeek) => number | null): Bar[] =>
    weeks.map((w, i) => {
      const j = judge[i];
      return { label: weekLabel(w), value: j === undefined ? null : f(j) };
    });
  const charts: [string, string, (j: JudgeWeek) => number | null][] = [
    ["Smoke open rate per week", "Open rate", (j) => j.openRate],
    ["Smoke render match per week", "Render match", (j) => j.renderMatch],
    ["Smoke text F1 per week", "Text F1", (j) => j.textF1],
    ["Smoke crashes per week", "Crashes", (j) => j.crashes],
    ["Journeys passing per week", "Journeys passing", (j) => j.journeysPassing],
  ];
  return (
    <Stack spacing={2}>
      <Typography variant="body2" color="text.secondary">
        The newest nightly judge run of each week (its gates report); a week without one is missing.
      </Typography>
      {charts.map(([title, column, f]) => (
        <BarChart key={title} title={title} column="Week" valueColumn={column} bars={bars(f)} />
      ))}
    </Stack>
  );
}
