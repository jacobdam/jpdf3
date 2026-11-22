// The web app's Content Security Policy (specs/ui-web/app-skeleton.md, docs/adr/0002).
// GitHub Pages can't send headers, so the build puts this in a <meta http-equiv> tag as the
// first element of <head>. Directives a meta tag ignores (frame-ancestors, report-uri,
// sandbox) are deliberately absent. Loosen a directive only with an ADR; the wasm engine
// will need 'wasm-unsafe-eval' in script-src and nothing more.
export const CSP = [
  "default-src 'none'",
  "script-src 'self'",
  // MUI's Emotion (production "speedy" mode) creates EMPTY <style> elements and fills them
  // through CSSOM insertRule, which CSP doesn't govern. The hash of the empty string allows
  // exactly those; a <style> with any text (an injected one) is still blocked.
  "style-src 'self' 'sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU='",
  "img-src 'self'",
  "font-src 'self'",
  "connect-src 'self'",
  "manifest-src 'self'",
  "base-uri 'none'",
  "form-action 'none'",
  "object-src 'none'",
  "require-trusted-types-for 'script'",
  "trusted-types 'none'",
].join("; ");
