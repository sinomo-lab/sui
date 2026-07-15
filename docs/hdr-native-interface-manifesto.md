# SUI HDR-Native Interface Manifesto

## Companion Documents

- [Documentation Index](./README.md)
- [SUI Design](./design.md)
- [HDR and Wide-Gamut Display Roadmap](./plans/hdr-wide-gamut-display-proposal.md)
- [Rendering Architecture](./renderer-architecture.md)
- [Testing Guide](./testing.md)
- [HDR Theme Tokens](./hdr-theme-token-schema-proposal.md)

This document describes the intended product and visual-language direction for HDR-native user interfaces in SUI. It is still primarily a vision document, but it now sits beside a first working milestone in code. Today that milestone is intentionally narrow: the HDR token schema exists, `Button`, `Switch`, and `Popover` are the first built-in widgets consuming it, and `sui-demo` exposes validation surfaces for the initial rollout.

## Current milestone reality

The manifesto below is broader than the current implementation. When reading it, keep this distinction in mind:

- **implemented now:** token schema, lightweight resolver helpers, pilot widget adoption, widget-book lab, and dev-host inspection controls
- **not implemented yet:** a full HDR-native application shell, broad widget adoption, ambient/context-aware luminance adaptation, and richer material systems across the whole toolkit

## Why SUI should care about HDR-native UI

SUI is aimed at demanding creative and technical software: editors, inspectors, timelines, node graphs, visualization tools, color tools, and graphics-heavy application shells. Those applications are exactly the kinds of software that can benefit from:

- wide-gamut color for richer, more faithful themes and graphics
- HDR headroom for highlight accents, indicator lights, stylized materials, and attention cues
- better integration between tool UI and HDR-authored content
- a rendering model that does not immediately collapse all advanced display behavior back to SDR assumptions

The goal is not to make every widget bright. The goal is to make SUI capable of a visual language that feels native to modern wide-gamut and HDR-capable displays.

## Vision

SUI should treat HDR as a first-class design dimension, but not as a decorative gimmick.

The ideal SUI HDR interface is:

- **reference-white by default** for readability and compositional stability
- **wide-gamut throughout** so ordinary UI color can be richer and cleaner without becoming louder
- **selectively emissive** so only the right things become brighter than SDR white
- **material-aware** so stylized widgets can look lit, translucent, glossy, or energetic without relying on crude fake glow
- **adaptive** to display capability, ambient context, and content context
- **accessible and disciplined** so text and interaction clarity always outrank spectacle

In short:

> SUI should enable interfaces that are calm by default, vivid where justified, and luminous only when meaningfully earned.

## Core thesis

HDR-native UI is not "normal UI plus more brightness."

It is a different discipline built around three separate but related ideas:

1. **Wide gamut** gives the interface a larger and cleaner color vocabulary.
2. **High dynamic range** gives the interface limited access to brightness above SDR white.
3. **Material and composition control** decide where that brightness should appear, how it should move, and how it should coexist with text, content, and the rest of the scene.

SUI should model those axes explicitly rather than collapsing them into a single vague concept of "HDR styling."

## First principles

### 1. Reference white is the anchor

SUI should follow the same practical mental model seen in Apple EDR and broadcast HDR graphics guidance:

- ordinary UI white corresponds to **reference white**
- most persistent UI chrome should live near that anchor
- values above reference white should be deliberate exceptions

This gives the interface a stable visual center and prevents brightness inflation.

### 2. Wide gamut is the baseline upgrade

Wide gamut should not be treated as an exotic special effect. It should become the default expressive substrate for:

- theme accents
- swatches and gradients
- status colors
- illustrations and vector surfaces
- editor and visualization content

A wide-gamut-native UI can look richer and more precise even when it is not especially bright.

### 3. HDR headroom is a scarce semantic resource

Brightness above reference white should be spent intentionally.

SUI should reserve HDR headroom primarily for:

- light-like indicators
- energized selected states
- attention and urgency cues
- glints, edge highlights, and tiny specular accents
- stylized emissive materials and textures
- content-coupled UI that must stay legible over HDR media

Large-area HDR UI should be rare.

### 4. Readability outranks spectacle

If a choice improves dramatic effect but weakens legibility, contrast stability, or interaction clarity, it is the wrong default for SUI.

This applies especially to:

- text over HDR content
- overlays on dark scenes
- glows adjacent to small typography
- high-frequency pulsing states
- bright modal surfaces

### 5. HDR should be stateful, not constant

A mature HDR-native interface uses luminance dynamically.

The most effective luminous cues are usually:

- small
- local
- short-lived
- tied to state transitions
- tied to meaning

Permanent glow quickly becomes noise.

### 6. Material response is better than painted tricks

When SUI offers stylized HDR widgets, the best result will usually come from material behavior:

- dynamic edge highlights
- subtle emissive interiors
- sheen on interaction
- translucent or glass-like lift
- shader-driven pulses or sweeps

This is more convincing and more controllable than baking fake glow into all assets.

### 7. Adaptation matters

Human vision and display behavior are adaptive. Therefore HDR UI must also be adaptive.

SUI should assume that:

- available headroom can vary per display
- SDR white can vary by platform state
- ambient brightness changes the comfort and readability of polarity and luminance
- content brightness changes how strong overlays should be

A fixed HDR look is not enough.

## What HDR-native means in practice

SUI should not aim for a uniformly bright interface. It should aim for a layered brightness hierarchy.

### Layer 0: structural UI

This includes:

- app background surfaces
- panels
- inspectors
- list rows
- ordinary containers
- default borders

These should usually remain within SDR-like luminance ranges, even on HDR displays.

### Layer 1: interactive UI

This includes:

- standard buttons
- text fields
- tabs
- toggles
- sliders
- menus

These should remain mostly reference-white-based, with modest brightness shifts for hover, pressed, and focused states.

### Layer 2: semantic accents

This includes:

- active indicators
- recording/live/armed states
- selected tools
- active handles
- successful or warning highlights

These are appropriate places for restrained HDR lift.

### Layer 3: energized attention cues

This includes:

- popup summon cues
- subtle flashes to draw attention
- deadline/alert pings
- animated focus arrivals
- reveal effects on important state changes

These should use time-bounded emissive behavior and settle back quickly.

### Layer 4: stylized material widgets

This includes:

- glossy or glassy controls
- textured artistic controls
- premium control surfaces
- custom widgets meant to feel luminous or light-reactive

These should rely on structured material systems and shader behavior, not indiscriminate bloom.

## SUI HDR design principles

### Calm base, vivid accents, luminous exceptions

This should be the central style law.

A default SUI HDR theme should feel:

- calm in ordinary panel structure
- vivid in its color accents
- luminous only in explicitly chosen moments

### Constrained HDR is the default mode

SUI should treat **constrained HDR** as the likely default for general-purpose application interfaces.

That means:

- the UI can use some headroom above white
- but it should not compete with HDR content everywhere
- it should preserve comfort for long sessions
- it should be safe for dense professional tools

Full HDR UI should be a style choice, not the baseline assumption.

### Semantic luminance, not arbitrary luminance

Brightness should communicate meaning. Examples:

- an armed live-preview light can sit above white
- a background panel should not
- a critical warning pulse can briefly peak
- a normal button border should not

### Local emphasis beats global emphasis

SUI should prefer:

- a bright edge
- a bright glyph
- a bright tiny indicator
- a short sweep across a popup rim
- a focused highlight region

instead of making an entire card, bar, or dialog much brighter.

### HDR should reinforce hierarchy already present in layout and color

Luminance should not be asked to create hierarchy from nothing. The structure should already exist through:

- layout
- spacing
- size
- semantic color
- typography
- grouping

HDR should then reinforce those relationships.

## Proposed effect classes

SUI should eventually offer a small, disciplined vocabulary of HDR-native effect classes.

### 1. Emissive indicator

Use for:

- recording
- live connection
- armed output
- enabled monitoring
- sync health
- attention-required badge

Characteristics:

- tiny area
- saturated wide-gamut color
- mild brightness above white
- optional soft local halo only when justified

### 2. Focus glint

Use for:

- keyboard focus arrival
- pen-focus highlight
- selected tool activation
- slider/thumb focus

Characteristics:

- brief
- narrow edge or rim treatment
- spatially precise
- non-distracting

### 3. Alert pulse

Use for:

- popup appearance
- inspector attention request
- transient error/warning emphasis
- user-guidance moment

Characteristics:

- short temporal envelope
- one or two pulses, not continuous blinking
- should decay into a stable readable state

### 4. Material sheen

Use for:

- premium buttons
- stylized knobs
- textured cards
- display-like or lens-like widgets

Characteristics:

- shader-driven
- view or interaction responsive where appropriate
- primarily a surface-quality cue, not a status alarm

### 5. Luminous texture accent

Use for:

- decorative but meaningful stylized widgets
- branded surfaces
- synthesizer-like or instrument-like controls
- rich creative-tool affordances

Characteristics:

- texture and luminance work together
- should still survive SDR fallback gracefully
- should not hide hit targets or state boundaries

### 6. Exposure-aware overlay mode

Use for:

- captions over HDR content
- measurement overlays on images/video
- HUD-like technical overlays
- guides over bright visualization surfaces

Characteristics:

- may adjust text/backplate strategy to content brightness
- prioritizes legibility over purity of chrome styling

## Anti-principles

SUI should explicitly reject the following defaults.

### 1. Everything glows

A fully glowing UI is visually cheap, tiring, and destroys hierarchy.

### 2. Bright text everywhere

Very bright text over dark or HDR content can reduce comfort and perceived contrast rather than improve it.

### 3. Huge bright modal surfaces

Large-area brightness spikes are more fatiguing than local cues.

### 4. Saturation inflation as a substitute for design

Wide gamut should create cleaner color, not just louder color.

### 5. Bloom as the primary styling system

Bloom should be a consequence of genuine luminance structure, not the main design tool.

### 6. Fixed HDR behavior regardless of display

SUI should not assume one display class, one room condition, or one stable headroom value.

## Accessibility and comfort commitments

SUI HDR design must remain compatible with long-session professional use.

### Text rules

- body text should usually remain in a controlled SDR-like luminance range
- small text should not be the brightest object on screen
- text over dynamic content should use scrims, backplates, blur, or adaptive contrast where needed
- focus visibility must not depend on extreme brightness

### Motion and pulse rules

- avoid continuous flashing
- prefer slow, subtle energy in idle states
- reserve repeated pulse behavior for genuine urgency
- provide a path for reduced-motion and reduced-flash accessibility modes

### Contrast rules

- maintain semantic contrast guarantees independently of HDR embellishment
- never assume brightness alone guarantees readability
- validate both light and dark themes under varying ambient assumptions

### Fatigue rules

- default UI should be comfortable for hours of use
- HDR accents should remain sparse enough that they still feel special after prolonged sessions

## Color philosophy

SUI should treat color in three layers:

### 1. Semantic role

Examples:

- primary action
- selection
- success
- warning
- danger
- info
- neutral support

### 2. Gamut expression

The same semantic role may be authored for:

- SDR / sRGB output
- wide-gamut output

The wide-gamut version should be richer or cleaner, not necessarily brighter.

### 3. Luminance behavior

A semantic color may also have an HDR-aware variant for specific states:

- selected
- focused
- active
- alerting
- emissive

This means SUI should eventually distinguish:

- **color identity**
- **gamut identity**
- **luminance behavior**

rather than treating one RGBA token as sufficient for every presentation mode.

## The manifesto for built-in widgets

Built-in widgets should eventually be designed under the following assumptions.

### Buttons

- ordinary buttons remain mostly SDR-like
- primary buttons may use wide-gamut accents first
- only high-importance or stylized button families should gain emissive or glint behavior

### Inputs

- editing clarity outranks visual drama
- focus should be precise and stable
- insertion caret, selection, and IME surfaces should not rely on dazzling brightness

### Toggles and switches

- on-state indicators are excellent candidates for restrained emissive cues
- the label and track should remain readable without depending on glow

### Menus and popups

- popup arrival can use a subtle luminance pulse or rim sweep
- resting popup surfaces should settle into calm readability quickly

### Tabs and toolbars

- selected-state lift should be modest and semantic
- active tool indicators may be slightly brighter than white, but not noisy

### Panels and inspectors

- mostly calm structural surfaces
- wide gamut can improve subtle color relationships
- HDR should appear only in the most meaningful status moments

### Custom stylized widgets

- may adopt richer materials and HDR textures
- still must preserve clear state, target size, and semantics
- should degrade cleanly on SDR paths

## The manifesto for content-integrated UI

SUI is for creative and technical applications, so some UI will coexist directly with rich content.

### Over image/video/visualization content

SUI should support overlay strategies that can remain readable without flattening the content experience.

Preferred tools:

- adaptive backplates
- local blur or dimming
- exposure-aware text and stroke choices
- constrained HDR for overlay emphasis

### Over canvas/editor surfaces

SUI should allow overlay chrome to feel integrated without becoming visually confused with the document itself.

That means:

- selection outlines can be energetic
- handles can be emissive
- guides and rulers can stay calmer
- tool HUDs can adapt to local content brightness

## Display-adaptive doctrine

SUI should eventually expose design decisions that scale by capability.

### SDR displays

- preserve hierarchy
- preserve semantic color
- no broken or washed-out fallback
- avoid pretending SDR is HDR

### Wide-gamut SDR displays

- richer accent color
- better gradients and swatches
- ordinary UI still mostly reference-white-based

### HDR displays in constrained mode

- selective headroom for accents and meaning
- excellent default for dense professional tools

### HDR displays in full mode

- available for stylized apps, premium skins, content-coupled UIs, and intentional artistic systems
- should still preserve readability and fatigue constraints

## Validation doctrine

SUI should judge HDR-native interface quality by more than screenshots.

A good HDR UI should be evaluated against:

- readability in light and dark themes
- comfort over long sessions
- visibility of focused and selected states
- behavior on SDR fallback
- behavior on wide-gamut SDR displays
- behavior on true HDR displays with different headroom
- overlays on bright and dark content
- local dimming / OLED differences where applicable

## Design vocabulary SUI should adopt

SUI should consistently talk about:

- reference white
- headroom
- constrained HDR
- emissive accent
- semantic luminance
- material response
- exposure-aware overlay
- highlight budget
- display-adaptive styling

This vocabulary is more precise and more useful than generic terms like:

- punchy
- vivid
- glowing
- HDR look

## Final commitment

SUI should not pursue HDR to make interfaces louder.

SUI should pursue HDR so interfaces can become:

- more expressive
- more materially convincing
- more semantically legible
- better integrated with HDR content
- and more native to the displays people actually use now

The design target is not a neon theme.
The design target is a professional interface language that knows when to stay quiet and when to emit light.

## One-sentence summary

> SUI HDR-native design should be reference-white by default, wide-gamut throughout, selectively emissive where meaning demands it, materially rich where style benefits from it, and relentlessly disciplined about readability, comfort, and adaptive behavior.
