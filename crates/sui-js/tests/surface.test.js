// The broad widget and API surface, exercised without a platform.
import assert from "node:assert/strict";
import { test } from "node:test";
import { sui, start } from "./support.js";

function click(running, x, y) {
  running.handleEvent(
    sui.Event.pointer("down", new sui.Point(x, y), undefined, undefined, "primary", 1),
  );
  running.handleEvent(
    sui.Event.pointer("up", new sui.Point(x, y), undefined, undefined, "primary"),
  );
}

test("widget and API surface", () => {

  const text = new sui.State("Ready");
  const textRunning = start(sui.label(text), "State text");
  text.set("Updated");
  assert.equal(textRunning.pendingCount, 1);
  assert.equal(textRunning.drain(), 1);
  assert.equal(text.get(), "Updated");

  const sourceState = new sui.State(2);
  const doubledState = sourceState.select((value) => Number(value) * 2);
  const observedState = [];
  const stateSubscription = sourceState.watch((value) => observedState.push(value));
  sourceState.set(3);
  assert.equal(doubledState.get(), 6);
  assert.deepEqual(observedState, [3]);

  const animationZero = sui.AnimationValue.scalar(0);
  const animationOne = sui.AnimationValue.scalar(1);
  const transition = new sui.Transition(animationZero, animationOne, 1, {
    easing: "linear",
  });
  assert.equal(transition.sample(0.5).scalarValue, 0.5);
  const spring = new sui.Spring(0);
  assert.equal(spring.step(1, 1 / 60) > 0, true);
  const animated = new sui.AnimatedValue(animationZero, { duration: 1, easing: "linear" });
  animated.setTarget(animationOne);
  assert.equal(animated.tick(0.5), true);
  assert.equal(animated.value.scalarValue, 0.5);
  const animationTrack = new sui.AnimationTrack("card", "layer.opacity");
  animationTrack.addKeyframe(new sui.Keyframe(0, animationZero, { easing: "linear" }));
  animationTrack.addKeyframe(new sui.Keyframe(1, animationOne, { easing: "linear" }));
  const animationClip = new sui.AnimationClip("fade", 0, 1);
  animationClip.addTrack(animationTrack);
  const animationTimeline = new sui.AnimationTimeline(1);
  animationTimeline.addClip(animationClip);
  assert.equal(animationTimeline.sample(0.5)[0].value.scalarValue, 0.5);
  const animationPlayer = new sui.AnimationPlayer(animationTimeline);
  animationPlayer.play();
  assert.equal(animationPlayer.tick(0.25)[0].value.scalarValue, 0.25);
  const animationDocument = new sui.AnimationDocument("Card motion", animationTimeline);
  const decodedAnimation = sui.AnimationDocument.parse(animationDocument.toDocumentFormat());
  assert.equal(decodedAnimation.name, "Card motion");
  const animationEditor = new sui.AnimationEditor(decodedAnimation);
  assert.equal(
    animationEditor.addKeyframe(
      0,
      0,
      new sui.Keyframe(0.75, animationOne, { easing: "ease-out" }),
    ),
    true,
  );
  assert.equal(animationEditor.canUndo, true);
  assert.equal(animationEditor.undo(), true);

  let semanticButtonPressed = false;
  const semanticRunning = start(
    sui.button("Semantic save", { onPress: () => { semanticButtonPressed = true; } }),
    "Semantic testing",
  );
  semanticRunning.setInspectorTracing();
  const semanticSnapshot = semanticRunning.render();
  const semanticButton = semanticSnapshot.getOne({ role: "button", name: "Semantic save" });
  assert.equal(semanticButton.visible, true);
  assert.equal(semanticSnapshot.find({ role: "button" }).length, 1);
  semanticRunning.click(semanticButton);
  assert.equal(semanticButtonPressed, true);
  const semanticInspector = semanticRunning.inspect();
  assert.equal(semanticInspector.semanticsNodes.length, semanticInspector.semanticsCount);
  assert.equal(semanticInspector.eventRoutes.length, semanticInspector.eventRouteCount);

  const responsiveState = new sui.ResponsiveSidebarState();
  assert.equal(responsiveState.setExpanded(false), true);
  assert.equal(responsiveState.openOverlay(), true);
  assert.equal(responsiveState.overlayOpen, true);
  const masterState = new sui.MasterDetailState();
  assert.equal(masterState.showDetail(), true);
  assert.equal(masterState.route, "detail");
  assert.equal(sui.renderWidget(sui.adaptiveView(
    sui.label("Compact"),
    sui.label("Medium"),
    sui.label("Expanded"),
  )).commandCount > 0, true);
  assert.equal(sui.renderWidget(sui.constraintView([
    new sui.ConstraintCase(sui.label("Wide"), 800),
  ], sui.label("Fallback"))).commandCount > 0, true);

  const notifications = new sui.NotificationCenter();
  const notificationId = notifications.notify("Build complete", "All tests passed", {
    duration: 5,
    urgency: "polite",
  });
  assert.equal(notifications.size, 1);
  // A toast fades in, so its first frame may paint nothing; its semantics are
  // published at once.
  assert.equal(
    sui.renderWidget(sui.notificationHost(notifications)).semanticsNames.includes("Build complete"),
    true,
  );
  assert.equal(notifications.dismiss(notificationId), true);
  assert.equal(notifications.size, 0);

  const virtualModel = new sui.VirtualListModel("Rows", [
    new sui.VirtualListItem("1", "First row"),
    new sui.VirtualListItem("2", "Second row"),
  ]);
  assert.equal(virtualModel.size, 2);
  assert.equal(sui.renderWidget(sui.virtualList("Rows", virtualModel)).commandCount > 0, true);
  assert.equal(virtualModel.update(new sui.VirtualListItem("1", "Updated row")), true);
  assert.equal(virtualModel.append(new sui.VirtualListItem("3", "Third row")), true);
  assert.equal(virtualModel.size, 3);

  const canvasStroke = new sui.CanvasStroke(new sui.Color(0.2, 0.5, 0.9, 1), 2);
  const canvasShape = sui.CanvasShape.rect(
    new sui.Rect(10, 10, 80, 48),
    new sui.Color(0.2, 0.5, 0.9, 1),
    canvasStroke,
  );
  const canvasViewport = new sui.CanvasViewport(0, 0, 1, 0);
  assert.equal(sui.renderWidget(sui.canvas("Vector canvas", {
    shapes: [canvasShape],
    viewport: canvasViewport,
    desiredSize: new sui.Size(240, 160),
  })).commandCount > 0, true);
  assert.equal(sui.renderWidget(sui.canvasRuler(
    "horizontal",
    "Canvas ruler",
    new sui.Size(1024, 768),
    { viewport: canvasViewport, viewportSize: new sui.Size(240, 160) },
  )).commandCount > 0, true);

  const dragScope = new sui.DragScope();
  const dragHost = sui.dragDropHost(dragScope, sui.row([
    sui.draggable(dragScope, sui.button("Drag source"), "asset:brush", {
      previewLabel: "Brush asset",
    }),
    sui.dropTarget(dragScope, sui.button("Drop target"), { effect: "copy" }),
  ], { gap: 8 }));
  assert.equal(sui.renderWidget(dragHost).commandCount > 0, true);
  assert.equal(dragScope.active, false);

  const floatingWorkspaceState = new sui.FloatingWorkspaceState();
  const floatingWorkspace = sui.floatingWorkspace(floatingWorkspaceState, [
    new sui.FloatingView(
      "Inspector view",
      new sui.Rect(12, 12, 240, 180),
      sui.label("Inspector content"),
    ),
  ], { name: "Editor floating workspace" });
  assert.equal(sui.renderWidget(floatingWorkspace).commandCount > 0, true);
  const floatingViews = floatingWorkspaceState.views();
  assert.equal(floatingViews.length, 1);
  assert.equal(floatingWorkspaceState.setMaximized(floatingViews[0].id, true), true);
  assert.equal(floatingWorkspaceState.views()[0].maximized, true);

  const pixelState = new sui.PixelCanvasState();
  pixelState.brushColor = new sui.Color(0.2, 0.5, 0.9, 1);
  pixelState.brushSize = 2;
  pixelState.requestExport();
  const pixelCanvas = sui.pixelCanvas(pixelState, "Pixel editor", 4, 4, {
    desiredSize: new sui.Size(240, 180),
    fitOnFirstLayout: true,
  });
  assert.equal(sui.renderWidget(pixelCanvas).commandCount > 0, true);
  const pixelExport = pixelState.latestExport();
  assert.equal(pixelExport.width, 4);
  assert.equal(pixelExport.height, 4);
  assert.equal(pixelExport.rgba8.length, 64);

  assert.equal(sui.renderWidget(sui.overlayHost(sui.label("Overlay content"))).commandCount > 0, true);
  assert.equal(sui.renderWidget(sui.commandPalette(
    "Commands",
    sui.textInput("Search"),
    { open: false },
  )).commandCount >= 0, true);
  assert.equal(sui.renderWidget(sui.bottomSheet(
    "Build output",
    sui.label("Bottom content"),
    { open: false, height: 220 },
  )).commandCount >= 0, true);
  assert.equal(stateSubscription.unsubscribe(), true);
  sourceState.set(4);
  assert.equal(doubledState.get(), 8);
  assert.deepEqual(observedState, [3]);

  const theme = sui.Theme.dark();
  theme.setControlSize("small");
  theme.setAccent(new sui.Color(0.2, 0.5, 0.9, 1));
  theme.setColor("success", new sui.Color(0.1, 0.8, 0.3, 1));
  assert.equal(theme.color("success").green > 0.7, true);
  theme.setNumber("radius-md", 9);
  assert.equal(theme.number("radius-md"), 9);
  const configuredWindow = new sui.Window("Configured");
  configuredWindow.setInitialSize(new sui.Size(800, 600));
  configuredWindow.setInitialPosition(new sui.Point(40, 60));
  configuredWindow.removeIcon();
  configuredWindow.root(sui.button("Save"));
  const configuredApp = new sui.App();
  const configuredMessages = [];
  configuredApp.on("background.complete", (payload) => configuredMessages.push(payload));
  configuredApp.setTheme(theme);
  configuredApp.configureRendering({
    outputColorPrimaries: "display-p3",
    dynamicRange: "hdr",
    colorManagement: "prefer-hdr",
  });
  configuredApp.window(configuredWindow);
  const configuredRunning = configuredApp.start();
  const configuredHandle = configuredRunning.windowHandle(0);
  assert.equal(configuredRunning.uiHandle().emit("background.complete", "Loaded"), true);
  assert.equal(configuredRunning.drain(), 1);
  assert.deepEqual(configuredMessages, ["Loaded"]);
  configuredRunning.setInspectorTracing();
  configuredRunning.render();
  theme.setPreset("light");
  assert.equal(configuredRunning.pendingCount, 1);
  assert.equal(configuredRunning.drain(), 1);
  configuredRunning.tick(1);
  configuredRunning.requestRedrawAll();
  configuredRunning.wakeWindow(configuredHandle);
  configuredRunning.handleEventFor(
    configuredHandle,
    sui.Event.rawMouseMotion(new sui.Point(3, -2)),
  );
  configuredRunning.handleEventFor(
    configuredHandle,
    sui.Event.window(
      "moved",
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      new sui.Point(12, 24),
    ),
  );
  const inspector = configuredRunning.inspect();
  assert.equal(inspector.tracingEnabled, true);
  assert.equal(inspector.widgetCount >= 1, true);
  assert.equal(inspector.semanticsCount >= 1, true);
  assert.equal(inspector.eventRouteCount >= 1, true);
  configuredRunning.setRenderOptions(configuredHandle, {
    outputColorPrimaries: "display-p3",
    colorManagement: "prefer-wide-gamut",
  });

  const dockState = new sui.DockState(
    new sui.DockLayout(sui.DockNode.tabs(["101", "102"], "101")),
  );
  const dockWidget = sui.dockWorkspace(
    dockState,
    [
      new sui.DockPanelSpec("101", "Files", sui.label("Files panel")),
      new sui.DockPanelSpec("102", "Search", sui.label("Search panel")),
    ],
    { name: "Editor workspace" },
  );
  assert.equal(sui.renderWidget(dockWidget).commandCount > 0, true);
  assert.equal(dockState.activate("102"), true);
  assert.equal(dockState.hide("101"), true);
  assert.deepEqual(dockState.snapshot().hidden, ["101"]);
  assert.equal(dockState.show("101"), true);
  assert.deepEqual(dockState.snapshot().hidden, []);

  const simplePicker = sui.simpleColorPicker("Accent", {
    color: new sui.Color(0.25, 0.5, 0.75, 1),
    mode: "hsv",
    compact: true,
  });
  assert.equal(sui.renderWidget(simplePicker).commandCount > 0, true);

  const richDocument = new sui.RichDocument("# Report");
  assert.equal(richDocument.appendMarkdown("\n\nWaiting"), true);
  assert.equal(richDocument.lastUpdate().appendOnly, true);
  const attachmentId = richDocument.appendAttachment("trace.json", {
    mediaType: "application/json",
    source: "artifact:trace",
    sizeBytes: "128",
  });
  const extensionId = richDocument.appendExtension("tool-call", "Build", {
    body: "cargo test",
    status: "success",
    metadata: { exit_code: "0" },
  });
  assert.equal(BigInt(extensionId) > BigInt(attachmentId), true);
  const richDocumentWidget = sui.richDocumentView(richDocument);
  assert.equal(sui.renderWidget(richDocumentWidget).commandCount > 0, true);

  const checked = new sui.State(false);
  let toggled;
  const checkboxRunning = start(
    sui.checkbox("Enabled", { checked, onChange(value) {
      toggled = value;
    }}),
    "State boolean"
  );
  click(checkboxRunning, 32, 18);
  assert.equal(checked.get(), true);
  assert.equal(toggled, true);
  assert.equal(Array.isArray(toggled), false);

  const selected = new sui.State(0);
  let selectedIndex;
  let selectedValue;
  const radioRunning = start(
    sui.radioGroup("Priority", ["Low", "Medium", "High"], { selected, onChange(index, value) {
      selectedIndex = index;
      selectedValue = value;
    }}),
    "State number"
  );
  click(radioRunning, 20, 52);
  assert.equal(selected.get(), 1);
  assert.equal(selectedIndex, 1);
  assert.equal(selectedValue, "Medium");

  const customCalls = new Set();
  const custom = {
    name: "Consumer boundary",
    measure(constraints) {
      assert.equal(this, custom);
      assert.equal(constraints instanceof sui.Constraints, true);
      customCalls.add("measure");
      return constraints.clamp(new sui.Size(80, 24));
    },
    event(event, context) {
      assert.equal(this, custom);
      assert.equal(event instanceof sui.Event, true);
      assert.equal(context instanceof sui.EventContext, true);
      assert.equal(["capture", "target", "bubble"].includes(context.phase), true);
      context.setHandled();
      context.requestPaint();
      context.requestSemantics();
      context.setClipboardText("custom widget copied text");
      assert.equal(event.customKind, "consumer-probe");
      customCalls.add("event");
      return true;
    },
    paint(paint) {
      assert.equal(this, custom);
      assert.equal(paint instanceof sui.Paint, true);
      customCalls.add("paint");
      paint.fillRect(paint.bounds, new sui.Color(0.2, 0.4, 0.8, 1));
    },
    semantics(semantics) {
      assert.equal(this, custom);
      assert.equal(semantics instanceof sui.Semantics, true);
      customCalls.add("semantics");
      semantics.node("button", "Consumer boundary");
    },
  };
  const customSnapshot = sui.renderWidget(
    new sui.Widget(custom),
    sui.Event.custom("consumer-probe")
  );
  assert.equal(customSnapshot.commandCount > 0, true);
  assert.deepEqual([...customCalls].sort(), ["event", "measure", "paint", "semantics"]);

  const composite = {
    name: "Composite",
    measured: [],
    arranged: [],
    measureWithChildren(constraints, childSizes) {
      this.measured = childSizes.map((size) => [size.width, size.height]);
      return constraints.clamp(new sui.Size(
        Math.max(...childSizes.map((size) => size.width)),
        childSizes.reduce((height, size) => height + size.height, 0),
      ));
    },
    arrange(bounds, childSizes) {
      let y = bounds.y;
      this.arranged = childSizes.map((size) => {
        const childBounds = new sui.Rect(bounds.x, y, bounds.width, size.height);
        y += size.height;
        return childBounds;
      });
      return this.arranged;
    },
    paint(paint) {
      paint.fillRect(paint.bounds, new sui.Color(0.1, 0.1, 0.1, 1));
    },
  };
  const compositeSnapshot = sui.renderWidget(new sui.Widget(composite, [
    sui.sizedBox({ child: sui.label("First child"), width: 100, height: 24 }),
    sui.sizedBox({ child: sui.label("Second child"), width: 120, height: 28 }),
  ]));
  assert.deepEqual(composite.measured, [[100, 24], [120, 28]]);
  assert.equal(composite.arranged.length, 2);
  assert.equal(compositeSnapshot.semanticsNames.includes("First child"), true);
  assert.equal(compositeSnapshot.semanticsNames.includes("Second child"), true);
  assert.equal(compositeSnapshot.commandCount > 1, true);

  const modifiers = new sui.Modifiers(true, true, false, false);
  const pointer = sui.Event.pointer(
    "move",
    new sui.Point(4, 5),
    "9",
    undefined,
    undefined,
    undefined,
    "mouse",
    true,
    modifiers
  );
  assert.equal(pointer.modifiers.shift, true);
  assert.equal(pointer.modifiers.control, true);
  const keyboard = sui.Event.keyboard("A", "pressed", "KeyA", "a", false, false, modifiers);
  assert.equal(keyboard.modifiers.shift, true);
  const ime = sui.Event.ime("compositionUpdate", "abc", 1, 2);
  assert.equal(ime.cursorStart, 1);
  assert.equal(ime.cursorEnd, 2);
  const resized = sui.Event.window("resized", undefined, new sui.Size(640, 480));
  assert.equal(resized.size.width, 640);
  const scale = sui.Event.window(
    "scaleFactorChanged",
    undefined,
    undefined,
    2,
    192,
    new sui.Size(800, 600)
  );
  assert.equal(scale.scaleFactor, 2);
  assert.equal(scale.rawDpi, 192);
  assert.equal(scale.suggestedSize.height, 600);

  sui.Shader.saturationValuePlane(0.3, 1, "srgb");
  sui.Shader.saturationBar(0.3, 0.8, "srgb");
  sui.Shader.valueBar(0.3, 0.8, 1, "srgb");
  sui.Shader.alphaBar(new sui.Color(1, 0, 0, 1));
  sui.Shader.rgbChannelBar(new sui.Color(1, 0, 0, 1), 0, 1);

  assert.equal(new sui.ExternalBackendHandle("0").isEmpty, true);
  assert.equal(sui.ExternalSync.generation("7").value, "7");

  const mutablePoint = new sui.Point(1, 2);
  mutablePoint.x = 3;
  assert.equal(mutablePoint.x, 3);

  const portableExports = [
    "actionCard",
    "brushPreview",
    "commandGroup",
    "coverageDots",
    "dateTimeInput",
    "dock",
    "fixedPaneSplit",
    "framedField",
    "measuredBottomDock",
    "passwordInput",
    "placementBadge",
    "propertyRow",
    "sectionLabel",
    "sideSheet",
    "splitView",
    "switchView",
    "trailingSlotRow",
  ];
  for (const name of portableExports) {
    assert.equal(typeof sui[name], "function", `${name} should be exported`);
  }

  const brushColor = new sui.Color(0.2, 0.5, 0.9, 1);
  const brushSpec = new sui.BrushPreviewSpec(brushColor, 20, 0.6, "square");
  assert.equal(brushSpec.color.blue, 0.9);
  assert.equal(brushSpec.size, 20);
  assert.equal(brushSpec.opacity, 0.6);
  assert.equal(brushSpec.shape, "square");

  const portableWidgets = [
    sui.actionCard(
      "Open project",
      "Choose a recent workspace",
      {
        enabled: new sui.State(true),
        onPress: () => {},
      },
    ),
    sui.brushPreview("Brush", brushSpec, { size: new sui.Size(40, 40) }),
    sui.commandGroup(
      "Commands",
      [sui.button("Run"), sui.button("Stop")],
      {
        axis: "vertical",
        padding: 8,
        gap: 4,
        cornerRadius: 6,
        background: new sui.Color(0.1, 0.1, 0.1, 1),
        border: new sui.Color(0.3, 0.3, 0.3, 1),
      },
    ),
    sui.coverageDots("Coverage", 2, 4, { tone: "accent", maxDots: 4, showLabel: true, minWidth: 80 }),
    sui.dateTimeInput(
      "Appointment",
      {
        value: new sui.State("2026-07-15T12:30"),
        placeholder: "YYYY-MM-DD HH:mm",
        onChange: () => {},
      },
    ),
    sui.dock(
      sui.label("Dock body"),
      {
        top: sui.label("Dock top"),
        topHeight: 24,
        bottom: sui.label("Dock bottom"),
        bottomHeight: 24,
        fallbackWidth: 320,
        fallbackBodyHeight: 240,
      },
    ),
    sui.fixedPaneSplit(
      sui.label("Fixed"),
      sui.label("Divider"),
      sui.label("Flexible"),
      {
        axis: "horizontal",
        fixedPane: "first",
        fixedExtent: 96,
        dividerExtent: 1,
        fallbackFlexibleExtent: 240,
      },
    ),
    sui.framedField(
      sui.textInput("Framed input"),
      {
        name: "Framed input",
        description: "A field with shared chrome",
        padding: 8,
        minHeight: 36,
        fillWidth: true,
        focused: new sui.State(false),
        invalid: new sui.State(false),
      },
    ),
    sui.measuredBottomDock(
      sui.label("Measured body"),
      sui.label("Measured bottom"),
      {
        fallbackSize: new sui.Size(640, 640),
      },
    ),
    sui.passwordInput("Password", { value: new sui.State("secret"), placeholder: "Enter password", onChange: () => {} }),
    sui.placementBadge(new sui.State("Primary"), { tone: "accent", current: 2, target: 4, minWidth: 96 }),
    sui.propertyRow("Opacity", sui.label("100%"), { stacked: true, labelWidth: 96, controlWidth: 180, gap: 8 }),
    sui.sectionLabel("Appearance", { semanticName: "Appearance section", color: brushColor }),
    sui.sideSheet(
      "Inspector",
      sui.label("Inspector body"),
      {
        description: "Edit the current selection",
        open: new sui.State(true),
        modal: true,
        dismissOnScrim: true,
        placement: "right",
        width: 280,
        headerAction: sui.button("Help"),
        actions: [sui.button("Apply")],
        onDismiss: () => {},
      },
    ),
    sui.splitView(
      sui.label("First pane"),
      sui.label("Second pane"),
      {
        axis: "horizontal",
        name: "Workspace split",
        ratio: new sui.State(0.5),
        minFirst: 120,
        minSecond: 120,
        dividerThickness: 1,
        onChange: () => {},
      },
    ),
    sui.switchView([sui.label("First view"), sui.label("Second view")], { selected: new sui.State(1) }),
    sui.trailingSlotRow(sui.label("Row body"), sui.button("Edit"), { trailingWidth: 64, trailingHeight: 28, gap: 8 }),
  ];

  assert.equal(portableWidgets.length, portableExports.length);
  for (const [index, widget] of portableWidgets.entries()) {
    assert.equal(widget instanceof sui.Widget, true, `${portableExports[index]} should return Widget`);
    const snapshot = sui.renderWidget(widget);
    assert.equal(
      Number.isInteger(snapshot.commandCount),
      true,
      `${portableExports[index]} should render through the native addon`
    );
  }

  const auditExports = [
    "FloatingStackWindow",
    "floatingStack",
    "virtualScrollView",
    "reorderableList",
  ];
  for (const name of auditExports) {
    assert.equal(typeof sui[name], "function", `${name} should be exported`);
  }

  const floatingWindow = new sui.FloatingStackWindow(
    new sui.Rect(12, 16, 180, 80),
    sui.label("Floating window")
  );
  assert.equal(floatingWindow instanceof sui.FloatingStackWindow, true);

  const auditWidgets = [
    sui.floatingStack([floatingWindow], { name: "Floating workspace" }),
    sui.virtualScrollView(
      [sui.label("Virtual row one"), sui.label("Virtual row two")],
      {
        name: "Virtual results",
        padding: 8,
        gap: 4,
      },
    ),
    sui.reorderableList(
      "Layers",
      [sui.label("Background"), sui.label("Foreground")],
      {
        gap: 8,
        dragThreshold: 4,
        previewLabel: "Moving layer",
        onReorder: (item, from, to) => {
        assert.equal(Array.isArray(item), false);
        assert.equal(typeof from, "number");
        assert.equal(typeof to, "number");
      },
      },
    ),
  ];

  for (const [index, widget] of auditWidgets.entries()) {
    const exportName = auditExports[index + 1];
    assert.equal(widget instanceof sui.Widget, true, `${exportName} should return Widget`);
    const snapshot = sui.renderWidget(widget);
    assert.equal(
      Number.isInteger(snapshot.commandCount),
      true,
      `${exportName} should render through the native addon`
    );
  }

  let reorderArgs;
  const reorderRunning = start(
    sui.reorderableList(
      "Reorder callback",
      [
        sui.sizedBox({ child: sui.label("First"), width: 200, height: 40 }),
        sui.sizedBox({ child: sui.label("Second"), width: 200, height: 40 }),
        sui.sizedBox({ child: sui.label("Third"), width: 200, height: 40 }),
      ],
      {
        gap: 8,
        dragThreshold: 4,
        previewLabel: "Moving row",
        onReorder: (item, from, to) => {
        reorderArgs = [item, from, to];
      },
      },
    ),
    "Reorder callback"
  );
  reorderRunning.handleEvent(
    sui.Event.pointer("down", new sui.Point(20, 20), "51", undefined, "primary", 1)
  );
  reorderRunning.handleEvent(
    sui.Event.pointer("move", new sui.Point(20, 30), "51", undefined, undefined, 1)
  );
  reorderRunning.handleEvent(
    sui.Event.pointer("move", new sui.Point(20, 120), "51", undefined, undefined, 1)
  );
  reorderRunning.handleEvent(
    sui.Event.pointer("up", new sui.Point(20, 120), "51", undefined, "primary", 0)
  );
  assert.deepEqual(reorderArgs, [0, 0, 2]);
  assert.equal(Array.isArray(reorderArgs[0]), false);

  // Items are enabled unless their `enabled` argument is false.
  const segments = sui.renderWidget(sui.segmentedControl("View", [
    new sui.SegmentedControlItem("List"),
    new sui.SegmentedControlItem("Map", undefined, undefined, false),
  ]));
  const segmentDisabled = Object.fromEntries(
    segments.semanticsNodes.map((node) => [node.name, node.disabled]),
  );
  assert.equal(segmentDisabled.List, false);
  assert.equal(segmentDisabled.Map, true);
  const tools = sui.renderWidget(sui.toolPalette("Tools", [
    new sui.ToolPaletteItem("brush", "Brush"),
    new sui.ToolPaletteItem("eraser", "Eraser", false),
  ]));
  const toolDisabled = Object.fromEntries(tools.semanticsNodes.map((node) => [node.name, node.disabled]));
  assert.equal(toolDisabled.Brush, false);
  assert.equal(toolDisabled.Eraser, true);

  console.log("sui-js consumer boundary: ok");
});
