import CssBaseline from "@mui/material/CssBaseline";
import { ThemeProvider } from "@mui/material/styles";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { theme } from "./theme";

const root = document.getElementById("root");
if (root === null) {
  throw new Error("jpdf: #root element missing from index.html");
}

// The browser opens a dropped file itself (leaving the app) unless the page cancels the
// drop. Until the reader can open files (M4), drops anywhere in the window are ignored.
for (const type of ["dragover", "drop"] as const) {
  window.addEventListener(type, (event) => event.preventDefault());
}

createRoot(root).render(
  <StrictMode>
    <ThemeProvider theme={theme} noSsr>
      <CssBaseline enableColorScheme />
      <App />
    </ThemeProvider>
  </StrictMode>,
);
