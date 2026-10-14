---
version: alpha
name: Local Sessions
description: A small concert reservation desk with a warm paper surface and clear seat availability.
colors:
  primary: "#164C43"
  canvas: "#F4F1E8"
  surface: "#FFFFFF"
  ink: "#172D29"
  muted: "#52645D"
  on-primary: "#FFFFFF"
  primary-hover: "#103D35"
  error: "#8B2D22"
  error-surface: "#FFF0EC"
  border: "#C8D0C6"
  focus: "#8B2D22"
typography:
  display:
    fontFamily: "Georgia, serif"
    fontSize: "48px"
    fontWeight: 400
    lineHeight: 1.1
  heading:
    fontFamily: "system-ui, sans-serif"
    fontSize: "24px"
    fontWeight: 650
    lineHeight: 1.25
  body:
    fontFamily: "system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.4
rounded:
  control: "8px"
  card: "16px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "40px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "{spacing.md}"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.on-primary}"
    typography: "{typography.label}"
    rounded: "{rounded.control}"
    padding: "{spacing.md}"
  event-card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "{spacing.lg}"
  feedback-error:
    backgroundColor: "{colors.error-surface}"
    textColor: "{colors.error}"
    typography: "{typography.body}"
    rounded: "{rounded.control}"
    padding: "{spacing.md}"
---

## Overview

Use the visual logic of a small venue's printed program: warm paper, an editorial
headline, clear event details and an orderly reservation desk. This is a proposed
identity for fictional concerts. All assets are local; system fonts need no download.
The primary task is choosing a concert and reserving one seat, with visible access to
personal reservations and cancellation. Keep implementation and experiment controls
out of the product interface. Explain fictional demo credentials in the sign-in area.

## Colors

Canvas is the page; surface is the card/list background. Ink is main content; muted
is supporting text. Primary is the action color with on-primary text. Error pairs with
error-surface and explicit recovery text. Border separates regions; focus is the
visible keyboard outline. Border/focus tokens are prose-applied CSS roles rather than
upstream component properties. Availability always includes words and a seat count.

## Typography

Use one editorial h1 with display typography and a restrained system-font hierarchy.
Event names wrap naturally. Body and labels remain readable at narrow widths and
200% zoom. Reduce the display heading to 36px below 600px. Use tabular numerals for
seat counts. All controls have visible labels and semantic HTML names.

## Layout

Use a centered container up to 1120px wide, with 24px desktop gutters and 16px below
600px. The header pairs the product name with session controls. A short intro precedes
event cards; personal reservations form a distinct section below or beside the cards.
Use up to three event columns at wide widths, two at intermediate widths and one
below 600px. Controls and content wrap; the page must have no horizontal overflow
at 320px. Keep availability, event title and reserve action adjacent. No fixed overlays.

## Elevation & Depth

Prefer borders and tonal surfaces. Cards can have a subtle shadow:
0 2px 8px rgba(23,45,41,0.06). Do not layer extra panels or hover effects on static cards.

## Shapes

Cards use the card radius; inputs/buttons use control. Touch controls are at least
44px high. Decorative concert glyphs may use local CSS or inline SVG and are hidden
from assistive technology. Important actions always have text labels.

## Components

### Session form

Persistent Username and Password labels, ordinary inputs with autocomplete, a Sign in
button and the fictional Alice/Bob credential hints. Show the current principal and
Sign out once authenticated. On identity change immediately clear personal content.

### Concert card

Title, date, venue, available/capacity count and Reserve one seat. A sold-out card says
Sold out and disables creation. Signed-out users see a clear sign-in requirement.
A pending request retains context and disables its button. A policy cap comes from
the server; show active count/limit and keep cancellation usable when capped.

### Reservation history

List the principal's event name, short reservation reference, textual state and Cancel
reservation action. Cancelled history remains visible without a cancel action. Empty
history explains that choosing a concert creates the first reservation. Do not show
another principal's data while a refresh request is pending.

### Feedback and recovery

Use a persistent status region with aria-live=polite for success/loading and an alert
for actionable errors. Invalid credentials, sold-out seats and the phase-B limit have
specific messages. An uncertain network outcome preserves the reserve request key
for a safe retry. Keep keyboard focus visible with a 3px focus outline and 3px offset.
Respect reduced motion; any transitions are optional and at most 120ms.

## Do's and Don'ts

- Use textContent for server-supplied text and native form/button semantics.
- Display server-owned availability and allowance as current observations.
- Preserve loading, empty, success, error and signed-out states.
- Keep the same action hierarchy across desktop and mobile.
- Do not copy the numeric reservation limit into JavaScript or invent live statistics.
- Do not add payments, seat maps, filters, dialogs or navigation unrelated to this task.
