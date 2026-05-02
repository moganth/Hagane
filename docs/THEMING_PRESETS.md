# Theme Presets

This guide explains the preset-based theme system used by the installer UI.

The goal is simple:

- keep the installer logic and page behavior stable,
- let each preset change the visual style,
- make it easy for teammates to add new looks without editing core UI code,
- keep the default appearance available when no preset is selected.

The repository currently supports a preset called `caramel_latte`, which gives the installer a warm latte/tan/beige look while keeping the same core UI and interactions.

## How The Theme Flow Works

Theme presets are resolved in layers.

1. The manifest declares `theme.preset`.
2. The engine stores that preset in installer state.
3. The runner converts the preset into CSS payloads.
4. The shell injects the preset CSS into the page iframe.
5. The page HTML and JavaScript stay the same.
6. CSS changes the appearance.

That means the installer can look completely different without rewriting the page logic.

## What A Preset Changes

A preset can change things like:

- background color and panel tint
- button shape and fill style
- borders, spacing, and shadows
- font family
- banner treatment
- page-level accents for specific screens
- subtle motion and surface styling

A preset should not change core installer behavior.
That means navigation, validation, IPC, and step execution stay shared.

## What Stays The Same

These parts remain core and reusable:

- installer manifest parsing
- step runner and hook execution
- page navigation logic
- custom page widget behavior
- IPC message shapes
- validation rules

You should think of preset theming as a presentation layer, not a logic layer.

## File Layout

The theme files live under `ui/themes`.

Recommended structure:

```text
ui/
  themes/
    default/
      global.css
    caramel_latte/
      global.css
      theme.json
      pages/
        welcome.css
        license.css
        requirements.css
        install_dir.css
        components.css
        summary.css
        progress.css
        finish.css
        custom.css
```

### What Each File Means

- `global.css` applies the preset across the whole installer UI.
- `pages/<page>.css` adjusts the look of one page only.
- `theme.json` is optional metadata for humans and tooling.

If a preset has no page-specific CSS, the installer still works.
It just uses the global styling.

## Current Manifest Contract

The manifest can use the following theme pattern:

```yaml
theme:
  preset: "caramel_latte"
  accent_color: "#B9764D"
  accent_dark_color: "#8F5A3A"
  accent_light_color: "#F7E6D4"
  background_color: "#F5E7D7"
  surface_color: "#FFF7EE"
  text_color: "#33241B"
  text_muted_color: "#785E4D"
  border_color: "#D8BCA6"
  font_family: "'Trebuchet MS', 'Segoe UI', sans-serif"
  border_radius: 10
```

Rules:

- `preset` selects the visual theme pack.
- The other fields still work as overrides.
- If `preset` is missing, the installer falls back to the default look.
- If a preset exists but a field is omitted, the UI still uses the built-in default token values.

## How The Preset Is Applied

The runtime path is important.

1. `installer.yaml` is parsed into the manifest model.
2. Theme data is stored in installer state.
3. The runner sends theme data to the shell.
4. The shell injects CSS into the page iframe before the page renders.
5. Page JavaScript keeps handling validation, buttons, and events.

This is why the preset approach is safe: it changes the visual layer while preserving the core flow.

## Why CSS First, Not HTML Replacement

The recommended theme mechanism is CSS-first.

That gives you:

- less duplication,
- less risk of breaking page logic,
- easier maintenance for teammates,
- a stable DOM contract across all presets,
- simpler diffs when adjusting a theme.

You can still use small JavaScript helpers if a preset needs micro-adjustments, but the default strategy should be CSS.

Avoid replacing the core page HTML for a preset unless you truly need a special one-off experience.
That makes the system harder to maintain and easier to break.

## How To Use A Theme Preset In `installer.yaml`

Use the preset name in the `theme` block.

Example:

```yaml
theme:
  preset: "caramel_latte"
```

Optional overrides can be added below it:

```yaml
theme:
  preset: "caramel_latte"
  accent_color: "#B9764D"
  border_radius: 10
```

That means:

- the preset provides the overall look,
- the override fields fine-tune specific tokens.

## Example Walkthrough: Caramel Latte

The file [sdk/example/caramel_latte.yaml](../sdk/example/caramel_latte.yaml) shows the preset in action.

What it does:

1. Selects `caramel_latte` as the preset.
2. Uses warm tan and beige colors.
3. Uses a softer font and rounder surfaces.
4. Keeps the same installer flow and custom pages.
5. Still allows the existing color token system to override any individual value.

This is the pattern to copy when you create the next theme.

## Step-by-Step: Add A New Theme

Follow these steps when creating a new preset.

### 1. Choose a clear name

Use a lowercase, underscore-separated preset name.

Examples:

- `caramel_latte`
- `midnight_ember`
- `forest_mist`

Avoid spaces and punctuation.

### 2. Create the theme folder

Add a new folder under `ui/themes/<preset_name>`.

Example:

```text
ui/themes/midnight_ember/
  global.css
  pages/
    welcome.css
    summary.css
```

Start small. You do not need every page file on day one.

### 3. Define the global style layer

Put the broad look and feel in `global.css`.

Use it for:

- background gradients,
- buttons,
- card surfaces,
- shadows,
- borders,
- typography,
- banner styling.

This file should make the preset recognizable at a glance.

### 4. Add page-specific overrides only when needed

Use `pages/<page>.css` for page-specific polish.

Examples:

- a more dramatic welcome screen,
- a different summary card layout,
- a progress bar style,
- custom field borders for custom pages.

Do not repeat the same rule in every page file.
Keep shared styling in `global.css`.

### 5. Register the preset in the runner

The runner must know how to load the preset CSS files.

In practice, that means adding the preset to the theme bundle loader in [runner/src/main.rs](../runner/src/main.rs).

If the preset is not registered there, the installer can still parse the manifest, but the preset styling will not be shipped into the UI.

### 6. Use the preset in a manifest

Point the manifest at the new preset:

```yaml
theme:
  preset: "midnight_ember"
```

Then test the installer end to end.

### 7. Verify fallback behavior

Check that the installer still works if:

- `preset` is omitted,
- a page-specific CSS file is missing,
- only token overrides are provided.

The default UI should remain functional in all three cases.

## Design Rules For Future Themes

Use these rules so themes stay maintainable.

- Keep the page HTML unchanged unless there is a strong reason to change it.
- Prefer CSS over JS.
- Keep each theme visually distinct.
- Keep text readable and contrast high.
- Avoid loading remote assets.
- Keep assets local and packaged with the installer.
- Do not hard-code business logic in theme files.
- Treat themes as presentation packs, not application code.

## Recommended Team Workflow

When a teammate adds a new theme:

1. Duplicate an existing preset folder.
2. Rename it.
3. Change only the CSS variables and surface rules first.
4. Test the welcome page and summary page.
5. Add page-specific CSS only after the global look is stable.
6. Update the example manifest if the theme should be demoed.
7. Run a full build and launch the installer.

This avoids a common mistake: designing a theme from page files before the global visual system is stable.

## When To Use Tokens Versus Presets

Use tokens when you only want brand tuning.

Examples:

- change the primary accent color,
- change fonts,
- slightly adjust radius or borders.

Use presets when you want a full visual identity.

Examples:

- Caramel Latte,
- a dark industrial theme,
- a soft pastel theme,
- a high-contrast enterprise theme.

A good rule of thumb is this:

- tokens customize,
- presets transform.

## Current State Of The Repository

Right now the repository supports:

- `theme.preset` in the manifest,
- runtime preset delivery from the runner,
- CSS injection through the shell,
- a shipped `caramel_latte` example manifest,
- a clean folder structure for new presets.

That is enough to add more theme packs without changing the core installer flow.

## Practical Summary

If you want a new theme, do this:

1. Add a preset folder under `ui/themes`.
2. Put shared visual rules in `global.css`.
3. Put page-specific overrides in `pages/*.css`.
4. Register the preset in the runner.
5. Set `theme.preset` in `installer.yaml`.
6. Build and test.

That is the safe, scalable way to theme this installer.
