import Box from "@mui/material/Box";
import { useTheme } from "@mui/material/styles";
import Table from "@mui/material/Table";
import TableBody from "@mui/material/TableBody";
import TableCell from "@mui/material/TableCell";
import TableHead from "@mui/material/TableHead";
import TableRow from "@mui/material/TableRow";

export type Bar = { label: string; value: number | null };

const WIDTH = 600;
const HEIGHT = 160;
const GAP = 4;

/**
 * An SVG bar chart followed by a table of the same values (specs/tooling/dashboard.md, Page).
 * A null value has no bar and reads `missing` in the table: never 0.
 */
export function BarChart({
  title,
  column,
  valueColumn,
  bars,
}: {
  /** The chart's accessible name. */
  title: string;
  /** Header of the label column. */
  column: string;
  /** Header of the value column. */
  valueColumn: string;
  bars: readonly Bar[];
}) {
  const theme = useTheme();
  const max = Math.max(1, ...bars.map((b) => b.value ?? 0));
  const slot = bars.length === 0 ? WIDTH : WIDTH / bars.length;
  return (
    <Box>
      <Box
        component="svg"
        role="img"
        aria-label={title}
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        sx={{ width: "100%", maxWidth: WIDTH, height: "auto", display: "block" }}
      >
        <line
          x1={0}
          y1={HEIGHT - 0.5}
          x2={WIDTH}
          y2={HEIGHT - 0.5}
          stroke={theme.palette.text.secondary}
        />
        {bars.map((b, i) => {
          if (b.value === null) return null;
          const h = (b.value / max) * (HEIGHT - 8);
          return (
            <rect
              key={b.label}
              data-bar={b.label}
              x={i * slot + GAP / 2}
              y={HEIGHT - h}
              width={Math.max(1, slot - GAP)}
              height={h}
              fill={theme.palette.primary.main}
            />
          );
        })}
      </Box>
      <Table size="small" aria-label={`${title}: values`} sx={{ maxWidth: WIDTH }}>
        <TableHead>
          <TableRow>
            <TableCell component="th">{column}</TableCell>
            <TableCell component="th" align="right">
              {valueColumn}
            </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {bars.map((b) => (
            <TableRow key={b.label}>
              <TableCell>{b.label}</TableCell>
              <TableCell align="right">{b.value === null ? "missing" : b.value}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  );
}
