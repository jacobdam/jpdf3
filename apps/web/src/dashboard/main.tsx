import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider } from "@mui/material/styles";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { theme } from "../theme";
import { Dashboard } from "./Dashboard";
import { parseSnapshot, type Snapshot } from "./data";
import { parseReport, type ReportState } from "./judge";

// The metrics dashboard page (specs/tooling/dashboard.md). Its only requests are ./data.json,
// the snapshot `cargo xtask ci linux` copies next to it, and ../judge/report/report.json, the
// judge report the pages job copies into the site.

/** The snapshot, or null when it is missing (any non-2xx answer) or invalid. */
async function load(): Promise<Snapshot | null> {
  try {
    const res = await fetch("./data.json", { cache: "no-store" });
    if (!res.ok) return null;
    return parseSnapshot(await res.json());
  } catch {
    return null;
  }
}

/** The judge report: missing on a non-2xx answer or a network error, else parsed. */
async function loadReport(): Promise<ReportState> {
  let body: string;
  try {
    const res = await fetch("../judge/report/report.json", { cache: "no-store" });
    if (!res.ok) return { state: "missing" };
    body = await res.text();
  } catch {
    return { state: "missing" };
  }
  const report = parseReport(body);
  return report === null ? { state: "invalid" } : { state: "ok", report };
}

function Page() {
  const [snapshot, setSnapshot] = useState<Snapshot | null | undefined>(undefined);
  const [report, setReport] = useState<ReportState>({ state: "loading" });
  useEffect(() => {
    let live = true;
    void load().then((s) => {
      if (live) setSnapshot(s);
    });
    void loadReport().then((r) => {
      if (live) setReport(r);
    });
    return () => {
      live = false;
    };
  }, []);
  return <Dashboard snapshot={snapshot} report={report} />;
}

const root = document.getElementById("root");
if (root === null) {
  throw new Error("jpdf: #root element missing from dashboard/index.html");
}

createRoot(root).render(
  <StrictMode>
    <ThemeProvider theme={theme} noSsr>
      <CssBaseline enableColorScheme />
      <Page />
    </ThemeProvider>
  </StrictMode>,
);
