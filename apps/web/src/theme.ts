import { createTheme } from "@mui/material/styles";

// Tokens from specs/ui/wireframes-web.md §1 (sizes, typography, palette).
const fontFamily =
  'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

export const theme = createTheme({
  colorSchemes: {
    light: {
      palette: {
        background: { default: "#f5f5f5", paper: "#ffffff" },
        text: { primary: "#1f1f1f", secondary: "#5f5f5f" },
        primary: { main: "#1565c0", contrastText: "#ffffff" },
        error: { main: "#c62828" },
        info: { main: "#0b6bcb" },
        warning: { main: "#9a5b00" },
        success: { main: "#2e7d32" },
      },
    },
    dark: {
      palette: {
        background: { default: "#121212", paper: "#1e1e1e" },
        text: { primary: "#e8e8e8", secondary: "#b0b0b0" },
        primary: { main: "#90caf9", contrastText: "#0d1b2a" },
        error: { main: "#f28b82" },
        info: { main: "#7cc4fa" },
        warning: { main: "#ffb74d" },
        success: { main: "#81c784" },
      },
    },
  },
  shape: { borderRadius: 4 },
  spacing: 8,
  typography: {
    fontFamily,
    fontSize: 14,
    h6: { fontSize: "1.25rem", lineHeight: "1.75rem", fontWeight: 500 },
    subtitle2: { fontSize: "0.875rem", lineHeight: "1.25rem", fontWeight: 600 },
    body2: { fontSize: "0.875rem", lineHeight: "1.25rem", fontWeight: 400 },
    caption: { fontSize: "0.75rem", lineHeight: "1rem", fontWeight: 400 },
    button: { fontSize: "0.875rem", lineHeight: "1.25rem", fontWeight: 500, textTransform: "none" },
  },
});
