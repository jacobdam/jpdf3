import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import Paper from "@mui/material/Paper";
import Stack from "@mui/material/Stack";
import Tooltip from "@mui/material/Tooltip";
import Typography from "@mui/material/Typography";

// The control boundary color (wireframes-web.md §1.3), the same in both appearances.
const CONTROL_BOUNDARY = "#8a8a8a";
const OPEN_REASON = "Opening files is not available yet";

/** W-02 start screen with no recent files (specs/ui/wireframes-web.md). No PDF logic here. */
export function App() {
  return (
    <Box component="main" sx={{ maxWidth: 960, mx: "auto", mt: 6, px: 3 }}>
      <Stack spacing={3}>
        <Typography variant="h6" component="h1">
          jpdf
        </Typography>
        <Paper
          variant="outlined"
          sx={{
            border: `2px dashed ${CONTROL_BOUNDARY}`,
            borderRadius: 2,
            maxWidth: 480,
            py: 6,
            px: 3,
            textAlign: "center",
          }}
        >
          <Stack spacing={2} sx={{ alignItems: "center" }}>
            <Typography variant="body2">Drop a PDF here to open it</Typography>
            {/* Disabled controls keep focus and explain why (wireframes-web.md §0 rule 5). */}
            <Tooltip title={OPEN_REASON} describeChild>
              <Button
                variant="contained"
                aria-disabled="true"
                onClick={(event) => event.preventDefault()}
                sx={{
                  opacity: 0.6,
                  cursor: "default",
                  // A disabled control must not look interactive: no hover darkening or lift.
                  "&:hover": { bgcolor: "primary.main", boxShadow: "none" },
                }}
              >
                Open file
              </Button>
            </Tooltip>
          </Stack>
        </Paper>
        <Stack spacing={1}>
          <Typography variant="subtitle2" component="h2">
            Recent files
          </Typography>
          <Typography variant="body2" sx={{ color: "text.secondary" }}>
            Files you open appear here.
          </Typography>
        </Stack>
      </Stack>
    </Box>
  );
}
