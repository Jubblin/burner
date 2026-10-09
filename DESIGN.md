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
typography:
  heading-lg:
    fontFamily: Satoshi
    fontSize: 22px
    fontWeight: 700
  heading-md:
    fontFamily: Satoshi
    fontSize: 15px
    fontWeight: 700
  body:
    fontFamily: Satoshi
    fontSize: 14px
    fontWeight: 400
  label:
    fontFamily: Satoshi
    fontSize: 12px
    fontWeight: 500
  mono:
    fontFamily: JetBrains Mono
    fontSize: 13px
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
Burner is a calm, precise disk image tool. Creative north star: Industrial/Utilitarian, softened; a quiet lab-instrument feel. The memorable thing: **"It will never wipe the wrong drive."** Mode: Operate (task-focused, no marketing). Decoration is minimal: no gradients, no glow. Light theme only (desk, room light). Red only ever means "this will be erased".

## Colors
Restrained palette, warm-tinted neutrals.
- background `#F4F2EE`, surface `#FFFFFF`
- text `#1B1F24`, text-muted `#5F6670`
- primary `#0F6B5C` with on-primary `#FFFFFF`
- danger `#B3261E`: ONLY the target drive card and the final Write button
- success `#1E7A46`
- warning `#A35F00`: errors use warning plus an icon, never red

## Typography
- Satoshi (Fontshare, free commercial use; 400/500/700) for headings, UI, and body.
- JetBrains Mono (OFL; 400/500) for drive paths, sizes, and hashes.
- Both bundled locally in the app (works offline).
- Sizes: heading 22px and 15px, body 14px, label 12px.

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
