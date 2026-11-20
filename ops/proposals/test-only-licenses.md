# Proposal: allow MPL-2.0 for test and build tools that are never shipped

Protected file: CONSTITUTION.md §3 rule 10. Raised by #12 (web skeleton), see
docs/adr/0002-web-e2e-ids-and-site.md.

## Problem
Rule 10 lists the allowed licenses with no distinction between code we ship and tools we only
run. Two useful npm tools are MPL-2.0:
- `axe-core`: the automated WCAG audit that specs/ui/accessibility.md §5 requires in the
  Playwright harness. Without it we have only hand-written accessibility assertions.
- `lightningcss`: a required dependency of Vite 8, so we stay on Vite 7.
(Also seen: `minimatch` BlueOak-1.0.0 via ESLint, `caniuse-lite` CC-BY-4.0 via Babel. We
replaced those tools instead; no change is needed for them.)

## Proposed diff
```diff
 10. **Licenses:** MIT, Apache-2.0, BSD-2/3-Clause, Zlib, ISC, Unicode-3.0; OFL-1.1 for fonts only.
     The project is MIT.
+    MPL-2.0 is also allowed for test and build tools whose code never ends up in a shipped
+    artifact (web bundle, macOS app, crates); `apps/web` checks its production dependency
+    tree against the strict list and the full tree against the strict list plus MPL-2.0.
```

## Why it is safe
MPL-2.0 is file-level copyleft: obligations apply only when distributing the MPL files
themselves. Test tools are never distributed; the shipped bundle stays MIT/Apache/BSD/ISC.

## Risk if not applied
No automated WCAG audit on the web (accessibility.md §5 stays partly unmet; issues are found
only by hand-written assertions and the audit's vision review). Vite stays on the 7.x line,
which will eventually stop receiving fixes.

## Until the owner decides
We follow the rule as written: no MPL-2.0 packages anywhere in the lockfile.
