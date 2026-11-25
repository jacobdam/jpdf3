import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider } from "@mui/material/styles";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { theme } from "../theme";
import { Dashboard } from "./Dashboard";
import { parseSnapshot, type Snapshot } from "./data";

// The metrics dashboard page (specs/tooling/dashboard.md). Its only request is ./data.json,
// the snapshot `cargo xtask ci linux` copies next to it.

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

function Page() {
  const [snapshot, setSnapshot] = useState<Snapshot | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    void load().then((s) => {
      if (live) setSnapshot(s);
    });
    return () => {
      live = false;
    };
  }, []);
  return <Dashboard snapshot={snapshot} />;
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
