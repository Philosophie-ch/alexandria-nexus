# TODO: alexandria-web — Svelte Frontend for philosophie-bib

## Context

philosophie-bib needs a web frontend for interactive features (data tables, search UI, bibliography browsing). Rather than building this into the Rails portal, create a standalone Svelte project that compiles to static JS/CSS bundles. These bundles can be embedded in any page — including the portal's Rails views.

The project is called `alexandria-web` (not tied to bibliography specifically) since it may grow to include additional interactive components over time.

## Architecture

```
alexandria-web/               ← new project
├── src/
│   ├── main.ts               # Mount-point: reads config from data attributes
│   ├── components/
│   │   ├── BibDataTable.svelte
│   │   ├── SearchWidget.svelte
│   │   └── ...
│   └── lib/
│       └── api.ts             # philosophie-bib API client
├── vite.config.ts
├── package.json
├── tsconfig.json
└── dist/                      # Build output
    ├── alexandria.js           # Single bundle
    └── alexandria.css          # Extracted styles
```

**No SvelteKit.** No Node server at runtime. Just Svelte + Vite compiling to a static JS bundle. Zero runtime memory cost — it's just files.

### Build output

Vite configured in library mode to produce a single self-contained bundle:

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  build: {
    lib: {
      entry: 'src/main.ts',
      name: 'Alexandria',
      fileName: 'alexandria',
      formats: ['iife'],          // Single global bundle, no module system needed
    },
    cssCodeSplit: false,           // Single CSS file
  },
});
```

Output: `dist/alexandria.js` (~50-150KB) + `dist/alexandria.css`.

## How the portal uses it

### Option A: Serve from assets server (simplest)

Build output is deployed to the assets server (PhiloAssets). Portal loads it via a `<script>` tag:

```erb
<!-- In a Rails view or layout -->
<script src="<%= asset_url('alexandria/alexandria.js', asset_type: :js) %>"></script>
<link rel="stylesheet" href="<%= asset_url('alexandria/alexandria.css', asset_type: :css) %>">

<div id="bib-datatable"
     data-api-url="https://bib-api.philosophie.ch/api/v1"
     data-locale="<%= current_language %>">
</div>
```

The Svelte app auto-mounts on `#bib-datatable`, reads configuration from `data-` attributes.

**Precedent:** The portal already loads external JS this way (ShareThis SDK in `application.html.haml`).

### Option B: Serve from philosophie-bib itself

philosophie-bib serves the static files at a `/static/` path (axum's `ServeDir`). No extra server, no assets server involvement:

```
GET /static/alexandria.js
GET /static/alexandria.css
```

Portal loads from the philosophie-bib URL directly.

### Option C: Serve from its own subdomain

Deploy alexandria-web as a standalone static site at `https://alexandria.philosophie.ch`. Could be a simple nginx container or even GitHub Pages. Portal embeds via cross-origin `<script>` tag.

### Recommendation

**Start with Option B** (served by philosophie-bib). It's zero extra infrastructure — just add `tower-http`'s `ServeDir` to the axum router. The JS/CSS files are tiny. Move to Option A or C later if needed.

```rust
// In philosophie-bib's router setup
use tower_http::services::ServeDir;

router.nest_service("/static", ServeDir::new("static/"));
```

## Mount-point pattern

The Svelte entry point scans for known mount targets and initializes the appropriate component:

```typescript
// src/main.ts

import BibDataTable from './components/BibDataTable.svelte';
import SearchWidget from './components/SearchWidget.svelte';

const components = {
  'bib-datatable': BibDataTable,
  'alexandria-search': SearchWidget,
};

for (const [id, Component] of Object.entries(components)) {
  const el = document.getElementById(id);
  if (el) {
    new Component({
      target: el,
      props: { ...el.dataset },
    });
  }
}
```

Each component mounts on its own `<div id="...">`. A page can include one or many. If the div isn't present, nothing happens — safe to load the bundle globally.

## Portal integration: what changes in Rails

Minimal. No Webpacker/Stimulus involvement. Just:

1. Add the `<script>` and `<link>` tags to the layout (or specific pages)
2. Add a `<div>` with the right `id` and `data-` attributes where the component should appear
3. Done — the Svelte bundle handles the rest client-side

No Ruby rendering logic needed for these components. The portal provides the mount point and configuration; Svelte talks directly to philosophie-bib's API.

## Where does alexandria-web live?

As a sibling project under the bibliography directory or the top-level philosophie-ch workspace:

```
philosophie-ch/
├── portal/legacy/
├── philoAssets/
├── sysadmin-utils/
├── bibliography/
│   ├── bib-sdk/
│   ├── bib-enhancer/
│   ├── philosophie-bib/
│   └── alexandria-web/        ← here (bibliography-adjacent)
```

Or at the top level if it grows beyond bibliography:

```
philosophie-ch/
├── portal/legacy/
├── philoAssets/
├── sysadmin-utils/
├── bibliography/
└── alexandria-web/            ← here (project-wide frontend)
```

**Decision depends on scope:** if it stays bibliography-focused, keep it under `bibliography/`. If it becomes the general interactive frontend layer for philosophie.ch, promote it to top-level.
