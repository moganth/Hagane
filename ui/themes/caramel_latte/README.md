# Caramel Latte Theme

This theme is structured so presentation assets stay fully isolated from global UI templates.

## Folder Layout

- `css/global.css`: Shared visual tokens and shell-level styling for the theme.
- `css/pages/*.css`: Page-scoped styles.
- `html/*.html`: Theme-owned page templates.
- `js/*.js`: Theme-owned behavior scripts used by themed HTML pages.

Current page overrides:

- `html/progress.html`
- `css/pages/progress.css`
- `js/progress.js`

Other pages currently use global HTML templates plus page CSS overrides in `css/pages/`.
