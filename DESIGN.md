---
# gstack: design-md-format=spec
name: Burner
description: Calm, precise disk image tool; red only ever means "this will be erased".
colors:
  background: "#F4F2EE"
  surface: "#FFFFFF"
  text: "#1B1F24"
  text-muted: "#5F6670"
  primary: "#0F6B5C"
  on-primary: "#FFFFFF"
  danger: "#B3261E"
  success: "#1E7A46"
  warning: "#A35F00"
  dark-background: "#16181B"
  dark-surface: "#1F2227"
  dark-text: "#ECEAE6"
  dark-text-muted: "#9AA0A8"
  dark-primary: "#4FBFA9"
  dark-primary-hover: "#6ACDB9"
  dark-primary-active: "#3AA891"
  dark-on-primary: "#0B1F1B"
  dark-danger: "#FF6B61"
  dark-danger-hover: "#FF857C"
  dark-danger-active: "#E5564D"
  dark-on-danger: "#1A0806"
  dark-success: "#4CC081"
  dark-warning: "#E8A33D"
  dark-border: "#32363D"
  dark-border-hover: "#4B515A"
  dark-progress-track: "#2A3A37"
  dark-disabled: "#2B2F35"
  dark-disabled-text: "#7C828A"
typography:
  heading-lg:
    fontFamily: JetBrains Mono
    fontSize: 20px
    fontWeight: 500
  heading-md:
    fontFamily: JetBrains Mono
    fontSize: 16px
    fontWeight: 500
  body:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: 400
  label:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: 500
  mono:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: 400
rounded:
  sm: 4px
  md: 8px
  lg: 12px
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.md}"
    padding: 8px 16px
  button-danger:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.md}"
    padding: 8px 16px
  drive-card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.lg}"
    padding: 16px
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: 8px
  step-header:
    textColor: "{colors.text}"
    padding: 8px 0
  progress-bar:
    backgroundColor: "{colors.primary}"
    rounded: "{rounded.sm}"
---

# Burner Design System

## Overview
Burner is a calm, precise disk image tool. Creative north star: Industrial/Utilitarian, softened; a quiet lab-instrument feel. The memorable thing: **"It will never wipe the wrong drive."** Mode: Operate (task-focused, no marketing). Decoration is minimal: no gradients, no glow. Light is the default theme (desk, room light). Red only ever means "this will be erased".

## Colors
Restrained palette, warm-tinted neutrals.
- background `#F4F2EE`, surface `#FFFFFF`
- text `#1B1F24`, text-muted `#5F6670`
- primary `#0F6B5C` with on-primary `#FFFFFF`
- danger `#B3261E`: ONLY the target drive card and the final Write button
- success `#1E7A46`
- warning `#A35F00`: errors use warning plus an icon, never red

### Dark theme
Light is the default. Dark follows the OS setting (`prefers-color-scheme`) with a manual override in settings (app decision, to be confirmed in the UI task). Tokens are the `dark-` colors in the front matter. Danger remains the only red. No glow on dark: depth comes from 1px borders (`dark-border`, `dark-border-hover`) and surface tint only.
- Contrast on dark-background: text 14.8, muted 6.8, primary 7.9, danger 6.4, success 7.8, warning 8.3.
- The dark danger button uses near-black `dark-on-danger` text; white on the lifted red is only 2.8.
- Progress track `dark-progress-track`; disabled fill `dark-disabled` with `dark-disabled-text`.

## Typography
- JetBrains Mono (OFL 1.1; 400/500) is the only typeface: headings, UI, body, and data (drive paths, sizes, hashes).
- Bundled locally in the app (works offline).
- Sizes: heading 20px and 16px, body 13px, label 12px, data 12px. Headings differ from body by size and colour as well as weight (500 vs 400); there is no bold file, so no faux bold.

## Layout
Grid-disciplined single-column wizard: 1 Source, 2 Target, 3 Write. Step numbers always visible. Fixed narrow window, about 560px. Plain top row of mode tabs: Write / Backup / Download OS. Spacing is a 4px base (4, 8, 16, 24, 32, 48) with 24px between steps.

## Elevation & Depth
Depth via 1px borders and tints only. No zero-offset glow, no shadows as decoration.

## Shapes
Rounded: sm 4px, md 8px, lg 12px.

## Components
Each documents hover, focus-visible, active, and disabled states.
- **button-primary**: primary fill, white text. Hover darkens slightly; focus-visible shows a 2px primary outline with 2px offset; active darker still; disabled gray.
- **button-danger**: danger fill, white text; used only for the final Write. Disabled (gray) until the user types or ticks the drive name ("Erase <drive name>").
- **drive card**: large drive name, then size, bus type, and full device path in mono. The target card uses danger accents.
- **input**: surface fill, 1px border; hover darkens border; focus-visible 2px primary outline; disabled muted.
- **step header**: step number always visible, heading-md label.
- **progress bar**: primary fill on a tinted track; the only animated element besides step change.

## Motion
Minimal-functional: progress bar and step change only. 150-250ms, ease-out enter, ease-in exit, no bounce.

## Logo
A teal flame on a white disc, on a primary-teal rounded tile: "burn a disc". Source: `design/logo.svg` (app icon source: `design/icon-source.svg`).
- Colours: tile `primary` #0F6B5C, disc white, flame `primary`. The same tile is used in light and dark themes.
- Minimum size: 16px (flame and disc stay legible). Keep clear space of at least 1/8 of the tile width.
- Do: use the supplied SVG or PNGs unchanged.
- Don't: use red or orange (red only means "this will be erased"), add gradients, glow or shadows, recolour the flame, or stretch the tile.
- macOS: the tile is full-bleed with its own rounded corners (radius 28/128), not Apple's padded squircle template, so it can look slightly larger than neighbouring Dock icons.

## Do's and Don'ts
Do:
- Show the exact drive name and device path wherever a drive is shown.
- Keep Write disabled (gray) until the user types or ticks the drive name ("Erase <drive name>").
- Use red nowhere else.

Don't:
- Gradients, glows, cards inside cards, emoji.
- Red for generic errors.
- system-ui as primary font.

## Decisions Log
| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-10-09 | Initial design system created | Created by /design-consultation from product context (no competitor research) |
| 2026-10-09 | Added dark theme tokens; light stays default | Approved dark mockup; OS-follow with manual override, danger still the only red |
| 2026-10-19 | Added logo (flame on disc, teal tile) | Chosen from three options; no red, matches primary |
| 2026-10-10 | Replaced Satoshi with JetBrains Mono everywhere | Satoshi licence (ITF FFL v2.0) does not allow redistributing the font files in the public repo; JetBrains Mono is OFL 1.1 |
