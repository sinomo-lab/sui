# SUI Design

## Companion Documents

- [Documentation Index](./README.md)
- [Architecture Overview](./architecture.md)
- [Stack Hosts and Bounds Note](./stack-hosts.md)
- [Crate Guide](./crate-architecture.md)
- [Text System Direction](./text-system.md)
- [Rendering Architecture](./renderer-architecture.md)
- [Testing Guide](./testing.md)
- [HDR-Native Interface Manifesto](./hdr-native-interface-manifesto.md)
- [HDR Theme And Token Schema Proposal](./hdr-theme-token-schema-proposal.md)

The documents above describe the current codebase. This file remains the higher-level design target for the framework.

## Vision

SUI is an event-driven UI toolkit for building demanding creative and technical applications such as vector editors, image editors, video tools, node graph editors, and scientific visualization software.

The toolkit should provide a strong foundation for applications that need:

- high-performance GPU rendering
- precise input handling
- rich 2D graphics primitives
- scalable text and layout
- large or effectively infinite workspaces
- embedding and interoperability with external systems

SUI is primarily designed for Rust and wgpu, with support for desktop, mobile, and WebAssembly targets. The core implementation should also be exportable to Python and JavaScript so applications can adopt SUI outside the Rust ecosystem.

## Goals

- Provide an event-driven UI architecture suitable for interactive, graphics-heavy software.
- Build on top of wgpu so rendering remains portable across native and web targets.
- Support desktop, mobile, and WASM without splitting the framework into unrelated codepaths.
- Support first-class language bindings for Python and JavaScript without making Rust the only viable application entry point.
- Offer a practical default layout system that is simple for common cases, but keep it as an optional utility layer rather than a mandatory framework-wide rendering concept.
- Provide first-class support for vector graphics, pixel graphics, text, and large canvas workflows.
- Provide strong accessibility support for built-in widgets and a clear accessibility interface for custom widgets.
- Provide debugging and automated UI testing support suitable for interaction-heavy applications.
- Support multi-threaded rendering where the target platform and runtime model allow it.
- Make the toolkit extensible enough for specialized tools such as custom brushes, node editors, visualization panels, and embedded external content.
- Keep collaboration-oriented application designs viable by avoiding core architectural choices that become roadblocks for synchronized multi-user workflows.
- Keep the core framework focused on reusable building blocks rather than application-specific policy.

## Non-Goals

- SUI does not aim to be a browser replacement or a full HTML/CSS engine.
- SUI does not own localization strategy; instead, it must integrate cleanly with user-provided i18n solutions.
- SUI does not try to ship a complete content creation application. It provides primitives and infrastructure for applications to build on.
- SUI does not attempt to expose every possible GPU abstraction in the core UI layer. Low-level access should exist where necessary, but the primary goal is smooth integration, not becoming a general-purpose rendering engine.
- SUI does not provide built-in collaboration protocols, CRDT implementations, shared-state engines, or network communication layers.

## Design Principles

### 1. Event-driven by default

SUI should model UI interaction around explicit events, state transitions, focus rules, and invalidation. This makes application behavior predictable and allows expensive work to be scheduled deliberately rather than hidden behind implicit polling.

### 2. Fast paths for common cases

Most widgets should work with a simple one-pass layout and a straightforward rendering model. Advanced controls may opt into more flexible or multi-pass behavior, but the default path must stay easy to reason about.

### 3. Graphics-native rather than form-native

Many UI frameworks are optimized for form entry and document layout. SUI should instead optimize for canvases, overlays, rulers, inspectors, timelines, editors, and tool-centric interactions while still supporting standard widgets.

### 4. Cross-platform without lowest-common-denominator design

Desktop, mobile, and web should share the same conceptual model. Platform differences should be isolated behind runtime and integration layers instead of leaking into every widget or rendering API.

### 5. Extensible subsystems

Subsystems such as brushes, render surfaces, text shaping, embedded content, layout behavior, and measurement/composition utilities should be replaceable or augmentable where practical.

### 6. Incremental complexity

The framework should be useful early with a minimal subset, while its architecture leaves room for advanced features such as infinite canvas, render-to-texture, and custom GPU pipelines.

### 7. Bindings-aware core

The core architecture should assume that some applications will drive SUI from Python or JavaScript. Public APIs, ownership boundaries, error handling, async behavior, and callback models should therefore be designed so they can be exported cleanly rather than relying on Rust-only patterns everywhere.

### 8. Semantics are first-class

Accessibility metadata should not be an afterthought layered onto widgets later. SUI should treat semantic information as part of the widget contract so built-in widgets are accessible by default and custom widgets can participate in the same model.

### 9. Collaboration-aware architecture

SUI does not need to implement collaboration itself, but it should avoid architectural choices that make synchronization unnecessarily difficult. Event flow, state updates, and invalidation behavior should remain explicit and inspectable so applications can integrate collaboration systems without fighting the framework.

## Default Widget Style

The built-in widgets in `sui-widgets` should ship with a clear default visual language rather than looking like unstyled placeholder controls.

The default style should balance three constraints:

- modern enough to feel current next to web-inspired application shells and inspector panels
- compact enough for dense professional tool UIs
- touch-safe enough that the same controls remain comfortable on tablets and other direct-input surfaces

That leads to the following baseline rules for first-party widgets:

- Use a restrained neutral light palette with a small number of high-contrast accent colors. The default should feel crisp and contemporary rather than heavy or overly stylized.
- Use rounded geometry and subtle borders by default, but keep the radii restrained. Controls should read as intentionally designed surfaces, not raw rectangles or overly soft pills.
- Default body typography should stay around `12px / 16px` equivalent sizing so dense panels remain readable without feeling oversized.
- Interactive controls should default to roughly `24px` minimum height. This keeps inspector and property panels compact while still preserving a comfortable hit target for touch and pen input.
- Small visual elements such as checkbox indicators or drag affordances must not become tiny click targets. The visible glyph may stay compact, but the interactive row or surrounding surface should provide the larger target.
- Hover, pressed, and focused states should be distinct without relying on dramatic motion or heavy skeuomorphic shading. Focus visibility is mandatory and should survive both mouse and keyboard navigation.
- Text inputs should prioritize legibility and editing clarity: visible caret, readable placeholder styling, and strong focus treatment.
- Buttons should default to a primary-action style that feels usable out of the box, while still allowing applications to layer richer theming later.

These defaults are not meant to replace a future inherited theming system. They are the baseline that makes SUI usable before global theme propagation exists, and they should therefore live in first-party widgets rather than in example code alone.

The current public API for this is `DefaultTheme` in `sui-widgets` and the top-level `sui` facade. Applications can clone `DefaultTheme::default()` or start from `DefaultTheme::dark()`, adjust semantic color tokens, palette, metrics, or typography tokens, and apply the result to built-in widgets explicitly.

The broader theme container should also remain extensible. The top-level `Theme` type should support typed theme extensions so applications and third-party widget libraries can attach additional schema for their own widgets without needing SUI to predefine every possible theme field.

## Primary Use Cases

SUI is intended for applications such as:

- vector graphics editors
- raster and pixel art editors
- video and motion tools
- node graph editors
- data and scientific visualization tools
- custom design tools and domain-specific creative software

These applications share a set of requirements that heavily influence the design:

- large interactive surfaces
- zooming and panning workflows
- mixed vector, text, image, and overlay content
- accurate pointer, keyboard, and IME handling
- custom rendering and custom widgets
- accessible semantics for both users and automation systems
- strong control over performance and memory behavior

## System Overview

At a high level, SUI consists of the following layers:

1. Application and widget layer
2. Event, focus, and input system
3. Layout and composition system
4. Scene, paint, and rendering system
5. Asset and resource management
6. Platform integration layer

These layers should stay loosely coupled. Widgets should not need detailed knowledge of GPU resource lifetime, and the renderer should not need to know application semantics beyond paint commands, surfaces, and invalidation regions.

## Core Architecture

### Widget and view model

SUI should use a retained UI tree with explicit widget state and event handling. A retained structure is a better fit than a purely immediate model for complex editing tools because it provides stable identity for focus, selection, drag-and-drop, accessibility hooks, and incremental redraw.

Each widget should be responsible for:

- receiving and responding to events
- participating in layout
- producing paint output or child composition
- exposing semantic information such as focusability or text input behavior

Widgets should also be able to expose richer semantic roles, names, descriptions, state, and actions so the same model can serve accessibility tooling, automated testing, and future AI-driven integrations.

Widgets may be lightweight where possible, but the framework should not force every interaction into a stateless redraw loop if persistent state is the more practical model.

Animation policy should follow the same rule. Widgets own their local animation state, choose when transitions start or stop, and decide whether a change should repaint content or update presentation-only layer properties. The runtime should provide current time, animation-frame wakes, timer wakes, and invalidation routing, but it should not become a framework-owned easing or transition engine.

### Event system

The event system is central to the framework. It should support:

- pointer events
- keyboard events
- text input and IME composition
- focus and blur
- drag and drop
- window, surface, and platform lifecycle events
- custom application-defined events

Event routing should support capture, target, and bubble-style handling where useful, but the exact API should remain simpler than a browser DOM. The main requirement is deterministic delivery and clear ownership of input.

### Invalidation and updates

SUI should not rely on a diff-based UI model. Instead, it should use explicit dirtiness and invalidation driven by events and widget state changes. Widgets notify the runtime when they are dirty, and the runtime schedules the minimum required layout, paint, and redraw work.

The runtime should support incremental invalidation for:

- layout
- paint
- hit testing
- text shaping or text layout
- GPU resources and cached surfaces

This matters especially for infinite canvases, timelines, large tables, and editor-style inspectors.

## Layout System

The built-in layout pipeline should default to a one-pass model:

- parent provides constraints
- child computes size
- parent places child

This model should be efficient, easy to reason about, and sufficient for most controls.

It should also stay clearly separated from the renderer and from SUI's core identity. Built-in widgets will use it heavily, but SUI should treat it as a practical utility rather than assuming every serious UI can or should fit into one measurement system.

Some widgets will need more than this. SUI should allow widgets to opt into custom layout logic for cases such as:

- text measurement dependent layouts
- flexible or intrinsic sizing
- virtualized collections
- multi-pass container layout
- canvas-like free positioning
- UI attached to arbitrary spatial systems such as 3D objects or non-rectilinear surfaces

The framework should also provide layout utilities inspired by utility-first systems such as Tailwind CSS, but adapted to Rust APIs and runtime composition rather than string-based class authoring.

Those utilities should be callable independently of the standard widget-tree rendering path so a custom widget or host system can initiate measurement/composition work directly, mix it with arbitrary layout logic, or ignore it entirely when another system is a better fit.

## Rendering Model

### Rendering backend

wgpu is the primary rendering backend. This provides a consistent abstraction across Vulkan, Metal, DirectX, and WebGPU while keeping SUI aligned with modern GPU workflows.

The renderer should support:

- batched 2D drawing
- offscreen surfaces and render-to-texture
- cached layers or widget-scoped retained surfaces
- clip, transform, and opacity composition
- multi-threaded preparation of render work where platform and backend constraints allow it
- integration points for custom GPU passes

SUI should provide both high-level drawing primitives and renderer-owned extension points. Widgets may use framework-provided primitives such as shapes, text, and pixel or texture utilities, but ordinary widgets should still paint through the scene abstraction rather than taking direct ownership of raw `wgpu` objects.

### Scene and paint abstraction

Widgets should emit paint instructions into a scene or paint graph abstraction rather than issuing raw GPU commands directly. This allows:

- batching and optimization
- caching of repeated content
- target-specific rendering strategies
- tooling such as debug overlays, selection visualization, and repaint diagnostics

Low-level rendering hooks should still be available for advanced integrations, but the normal widget path should go through a stable paint abstraction.

For retained animation, that abstraction should stay presentation-oriented. Explicit paint-boundary widgets may expose portable layer properties such as opacity and translation, while the runtime and renderer decide how to diff and apply those updates efficiently. That keeps animation-friendly retained paths available without turning the widget API into a raw graphics abstraction layer.

Widgets own the lifecycle of their children. For general canvas and infinite-canvas workflows, SUI should treat the scene graph as a normal SUI widget tree rather than introducing a separate retained scene model that duplicates widget ownership.

## Graphics Capabilities

### Vector graphics

SUI should include built-in support for vector-oriented workflows:

- basic shapes
- transforms, scaling, and rotation
- paths and path editing primitives
- fills and gradients
- boolean operations

The design target is not only icon drawing or decorative UI, but editing-grade vector functionality that higher-level applications can build on.

Vector boolean operations belong in the core framework because they are a common requirement in the kinds of applications SUI is designed to serve.

### Pixel graphics

SUI should support pixel-accurate rendering and editing scenarios:

- pixel-perfect drawing
- scaled and transformed raster content
- pixel editing tools such as brush and eraser workflows
- layer composition and blend behavior
- an extensible brush engine, potentially including compute-shader-based implementations

This subsystem is important because many target applications blend UI and image editing in the same runtime.

### Text

Text is a first-class subsystem, not a thin utility. It should support:

- multilingual text rendering
- font loading and management
- text shaping and layout
- text input and editing
- artistic text and conversion to vector outlines where needed
- text area widgets and markdown rendering

Text input must support IME and platform text services. SUI should not rely on raw keyboard events for international text entry.

Basic text and font support belong in the core framework. Rich text editing, advanced text layout, and markdown-oriented features should live in a higher-level subsystem built on top of the core text stack. Outline conversion belongs in core because glyphs are already fundamentally vector data.

## Infinite Canvas

Infinite or very large canvas workflows are a core design target. The framework should support:

- large scrollable and zoomable workspaces
- chunk-based or other spatially partitioned rendering
- cached regions
- mipmap or level-of-detail strategies where useful
- selective redraw of changed regions

This capability should work across vector, raster, text, and embedded content rather than being limited to one rendering mode.

## Standard UI Controls

Although SUI is graphics-oriented, it still needs a practical set of standard widgets:

- buttons
- toggles, check boxes, and radio controls
- input fields
- sliders and numeric value adjustment controls
- tables and structured data views
- breadcrumbs and other navigation widgets
- grouping containers that can influence shared rendering or border treatment

These widgets should feel native to the framework's event, layout, and paint model rather than imported as a separate form toolkit.

Built-in widgets should provide decent accessibility behavior out of the box, including semantic roles, names, states, focus behavior, and actionable metadata where applicable.

## Accessibility and Semantics

Accessibility support should be a core design concern for SUI.

This includes traditional accessibility use cases such as screen readers, keyboard navigation, and platform accessibility services, but it is also valuable as a structured semantic layer for automation and AI integration. A well-defined semantic model makes it easier for tools to understand what the UI contains, what state it is in, and what actions are available.

The framework should provide:

- accessible semantics for built-in widgets by default
- a clear interface for custom widgets to expose semantic roles, labels, descriptions, values, states, and actions
- focus and navigation behavior that aligns with semantic structure
- integration points for platform accessibility APIs where available
- a stable semantic representation that can also be used by testing and automation tooling

Accessibility support should be practical rather than ornamental. If a widget is interactive, it should have a meaningful semantic representation unless there is a strong reason not to.

## Input and Interaction

### Drag and drop

Drag and drop should be built into the event model rather than implemented as a library-side workaround. It should support internal drag operations and platform integration where available.

### Keyboard navigation

Keyboard navigation should be part of the default focus system. Applications should be able to customize navigation order and behavior for tool-centric interfaces.

### Text input and internationalization support

SUI must support the requirements of multilingual applications, including:

- IME-aware text entry
- correct text shaping for multiple scripts
- application-level localization hooks
- platform-appropriate accessibility and input behavior where feasible

The framework should provide the necessary hooks and event semantics without dictating how applications manage translations or localized resources.

## Color and Media

### Color management

Color management should be treated as a real system concern, especially for graphics and editing software. The design should leave room for:

- multiple color spaces
- correct conversion between working and display spaces
- consistent handling between vector, text, image, and composited layers

Color management is a first-release requirement, not a later enhancement. SUI should implement a full color-management pipeline as early as possible, up to the practical limits of wgpu and the target platforms. This avoids baking incorrect assumptions into the renderer and reduces the risk of subtle output failures that only appear late in production workflows.

### Multimedia support

SUI should support media-oriented applications through:

- video playback
- audio playback
- playback control widgets
- waveform or wave visualizers

Media decoding may live outside the core UI runtime, but the framework should provide suitable surfaces, timing hooks, and widgets for integration.

## Embedding and Interoperability

SUI should support embedding content provided by external systems, including cases where an external library:

- provides a texture to display
- draws into a shared surface
- requires coordinated event handling
- needs to coexist with SUI overlays or hit testing

This is important for integrating specialized engines, media pipelines, HTML views, or third-party visualization components.

### Web and HTML integration

For WASM builds, SUI should support both:

- embedding SUI inside an HTML canvas or host page
- embedding or interoperating with HTML content where appropriate

This includes coordination of drag-and-drop, input focus, sizing, and event flow between SUI and DOM-managed elements.

Correct z-order between SUI and HTML content should be handled on a best-effort basis. Where platform constraints make strict ordering impossible, it is acceptable for one side to remain consistently on top. Input ownership should follow embedding direction: the host that embeds the other is responsible for primary input ownership and routing.

## Debugging and Testing

SUI should provide strong debugging support for both framework development and application development.

This should include:

- debug overlays and repaint diagnostics
- event tracing and input inspection
- layout and invalidation inspection tools
- resource and surface visibility for diagnosing rendering issues

SUI should also support automated UI testing with an approach similar in spirit to Playwright-style tests. Applications should be able to drive the UI through realistic input simulation, inspect visible state, and assert behavior at the widget and surface level.

The testing model should support:

- deterministic event injection
- querying widget identity, state, and semantics for assertions
- screenshot or surface-based regression checks where appropriate
- headless or test-harness-driven execution where possible
- validation of drag-and-drop, focus, keyboard navigation, and text input flows

The goal is to make UI unit and integration tests practical for interactive applications rather than treating them as an afterthought.

## Collaboration Considerations

SUI does not directly provide collaboration infrastructure such as CRDTs, distributed state management, presence systems, or transport layers. Those concerns belong to applications or higher-level frameworks.

However, SUI should be designed with collaboration in mind so it does not become a blocker for synchronized multi-user workflows.

This implies several architectural constraints:

- state changes should remain explicit enough to observe, serialize, and replay
- event and invalidation paths should avoid unnecessary high-frequency feedback loops that are tolerable locally but hostile to synchronization
- widget and rendering systems should make it practical to separate authoritative state from local presentation details
- automation, debugging, and semantic inspection tools should help applications reason about synchronized UI state

The goal is not to make every UI state automatically collaborative. The goal is to avoid coupling the framework to assumptions that only work in a single-user, purely local execution model.

## Theming and Styling

SUI should support theming as a first-class capability. Themes should control visual appearance without forcing applications into a rigid styling language.

The system should support:

- shared theme values
- local overrides
- state-based appearance changes
- consistent styling across standard widgets and custom drawing

Styling should be expressive enough for design tools and branded applications, but lightweight enough to remain practical for Rust developers.

## Basic 3D Support

SUI should provide a path for basic 3D integration, primarily by exposing or bridging raw wgpu functionality in a way that coexists cleanly with the UI renderer.

This does not mean the UI toolkit becomes a full 3D engine. The objective is smooth interoperability so applications can place 3D views, overlays, and controls inside the same runtime.

## Extensibility Model

SUI should be extensible in several directions:

- custom widgets
- custom layout containers
- custom paint operations
- custom GPU-backed rendering nodes
- custom input behaviors
- specialized editing tools such as brush engines or node graph surfaces

The core framework should define stable extension points early, because many target applications will need domain-specific behavior that does not belong in the base toolkit.

## Language Bindings

In addition to the native Rust API, SUI should provide supported bindings for Python and JavaScript.

These bindings are not just packaging work. They affect the shape of the core API and runtime. The design should therefore distinguish between:

- an internal Rust implementation API that can remain idiomatic and low-level
- a stable exported surface that is practical to call from Python and JavaScript

The exported surface should prioritize:

- explicit object lifetimes and ownership boundaries
- predictable error reporting
- callback and event registration patterns that are safe across FFI boundaries
- data structures that can be serialized, marshaled, or wrapped efficiently
- minimal dependence on Rust-specific type-system features in the public cross-language surface

Rust remains the owner of the core data model and execution model. Native Rust extensions may participate directly in those internals, but Python and JavaScript integrations must go through the exported API surface and an explicit communication layer. Mixed execution across these boundaries should be treated as potentially dangerous and designed conservatively.

Where possible, higher-level concepts such as widgets, scene updates, resources, and event dispatch should map cleanly across all supported languages, even if each binding exposes an API style that feels natural in that ecosystem.

Python and JavaScript bindings should expose the same high-level capabilities as the Rust API, with the main exception that low-level raw wgpu access may remain Rust-only. Reduced performance is acceptable in the bindings layer, and some execution models such as user-controlled multithreaded widgets may reasonably remain limited or unavailable outside Rust.

### Python bindings

Python bindings should make SUI usable for tool scripting, rapid prototyping, domain-specific applications, and integration into existing Python-based workflows.

The design should favor:

- a clear object model that feels natural in Python
- straightforward event subscription and callback registration
- practical access to rendering surfaces, assets, and widget state
- support for embedding SUI into Python-hosted applications without forcing Rust as the top-level application layer

Python bindings do not need to expose the same threading model as native Rust. It is acceptable for Python-controlled widgets or callbacks to run through a more constrained execution path as long as capability parity is preserved at the framework level.

Python performance-sensitive paths may still rely on Rust implementation internally, but the control surface should remain ergonomic for Python users.

### JavaScript bindings

JavaScript bindings should make SUI usable in browser-hosted and potentially other JS-driven runtimes, especially when targeting WASM.

The design should favor:

- APIs that map cleanly to asynchronous JavaScript execution models
- integration with browser event loops and DOM hosting environments
- clear ownership rules between JavaScript objects and WASM-managed resources
- compatibility with web embedding scenarios where SUI interoperates with HTML content

As with Python, JavaScript bindings may impose tighter execution constraints than native Rust, particularly around threading and direct low-level GPU access.

The JavaScript layer should not be treated as an afterthought on top of WASM export alone. It should be considered part of the supported interface design.

## Platform Targets

SUI is intended to support:

- desktop platforms as the primary initial target
- mobile platforms where touch, performance, and constrained screen layouts matter
- WebAssembly for browser-hosted tools and embeddable experiences

The design should prefer platform abstraction layers that preserve common behavior while still allowing platform-specific integrations for text input, drag-and-drop, windowing, and media.

## Performance Considerations

Performance is a design requirement, not a later optimization pass. The framework should aim for:

- minimal unnecessary layout and repaint work
- aggressive batching where it does not break correctness
- efficient handling of very large surfaces
- explicit caching of expensive intermediate results
- predictable memory behavior for retained surfaces, textures, glyph caches, and media surfaces
- multi-threaded render preparation and scheduling where safe and beneficial

This is especially important because the target applications often combine dense UIs with complex visual content.

## Expected Implementation Priorities

The framework should likely be developed in phases.

### Phase 1: Core runtime

- event loop and platform shell
- widget tree
- one-pass layout
- basic paint abstraction
- standard input handling
- a minimal set of core widgets
- color management architecture integrated into the renderer from the start
- semantic and accessibility interfaces integrated into the widget model from the start

### Phase 2: Graphics foundation

- vector drawing primitives
- text shaping and font management
- vector boolean operations in core
- outline conversion and core text primitives
- render-to-texture support
- multi-threaded render preparation
- basic theming
- drag-and-drop and keyboard navigation
- out-of-the-box accessibility behavior for core widgets
- debugging overlays and automated UI testing infrastructure

### Phase 3: Editor-oriented capabilities

- infinite canvas
- retained surface and cache infrastructure
- pixel editing surfaces
- advanced widgets such as tables and inspectors
- embedding of external render content

### Phase 4: Advanced integration

- WASM and HTML interoperability improvements
- mature Python and JavaScript bindings
- media integration
- color management expansion
- basic 3D interop
- richer extension points and tooling support

## Summary

SUI aims to be a Rust-native, wgpu-based, event-driven UI toolkit for graphics-heavy applications. Its core value is not just rendering widgets, but providing the architectural building blocks needed for serious interactive tools: efficient event routing, practical layout, strong text and graphics support, accessible semantics, infinite canvas workflows, clean integration with external systems, collaboration-friendly architecture, and a supported path to Python and JavaScript adoption.

The framework should start from a compact, coherent core and grow into advanced capabilities without losing clarity or performance discipline.
