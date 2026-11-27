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
import type { Milestone, Snapshot, Week } from "./data";

// specs/tooling/dashboard.md, Page. Snapshot text (issue titles) is rendered as React text.

export const INVALID = "No metrics snapshot: data.json is missing or invalid.";
export const NO_INDEX = "Missing: the snapshot has no session index.";

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

export function Dashboard({ snapshot }: { snapshot: Snapshot | null | undefined }) {
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
          <Sections s={snapshot} />
        )}
      </Stack>
    </Box>
  );
}

function Sections({ s }: { s: Snapshot }) {
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
    </>
  );
}
