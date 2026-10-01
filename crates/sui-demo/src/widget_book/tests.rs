use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

use sui::{
    App, DefaultTheme, Event, Point, PointerButton, PointerButtons, PointerEvent, PointerEventKind,
    Rect, RenderOutput, Result, Runtime, SemanticsNode, SemanticsRole, SemanticsValue, Size,
    SizedBox, Vector, Window, WindowEvent, WindowId,
};
use sui_testing::prelude::*;

use super::page::{PageItem, page_items, story_block};
use super::registry::{Category, StoryCtx, stories, story};
use super::shell::RAIL_SCROLL_NAME;
use super::{
    GALLERY_SCROLL_BAR_NAME, GALLERY_SCROLL_NAME, WIDGET_BOOK_NAV_NAME, WIDGET_BOOK_SEARCH_NAME,
    WIDGET_BOOK_THEME_SWITCH_NAME, WINDOW_TITLE, build_widget_book_application,
    build_widget_book_gallery, build_widget_book_gallery_with_theme, register_widget_book_images,
};
use crate::app::DemoTextRole;
use crate::test_support::*;

/// Themes every story must build and render in.
fn builtin_themes() -> [(&'static str, DefaultTheme); 5] {
    [
        ("light", DefaultTheme::sui()),
        ("dark", DefaultTheme::dark()),
        ("neutral", DefaultTheme::neutral()),
        ("neutral dark", DefaultTheme::neutral_dark()),
        ("void", DefaultTheme::high_contrast()),
    ]
}

fn open_book(size: Size) -> Result<TestApp> {
    let app = TestApp::new(|| build_widget_book_application().build())?;
    let window = app.main_window()?;
    window
        .root()
        .dispatch_event(Event::Window(WindowEvent::Resized(size)))?;
    window.run_until_idle()?;
    Ok(app)
}

fn node<'a>(
    nodes: &'a [SemanticsNode],
    role: SemanticsRole,
    name: &str,
) -> Option<&'a SemanticsNode> {
    nodes
        .iter()
        .find(|node| node.role == role && node.name.as_deref() == Some(name))
}

fn named<'a>(nodes: &'a [SemanticsNode], name: &str) -> Option<&'a SemanticsNode> {
    nodes.iter().find(|node| node.name.as_deref() == Some(name))
}

fn selected_rail_links(nodes: &[SemanticsNode]) -> Vec<String> {
    nodes
        .iter()
        .filter(|node| node.role == SemanticsRole::Link && node.state.selected)
        .filter_map(|node| node.name.clone())
        .collect()
}

fn gallery_bounds(nodes: &[SemanticsNode]) -> Rect {
    node(nodes, SemanticsRole::ScrollView, GALLERY_SCROLL_NAME)
        .expect("widget book gallery is present")
        .bounds
}

fn build_runtime<W>(size: Size, root: W) -> Result<(Runtime, WindowId)>
where
    W: sui::Widget + 'static,
{
    let runtime = App::new()
        .with_resources(|resources| {
            register_widget_book_images(resources);
            Ok(())
        })?
        .window(Window::new(WINDOW_TITLE).root(SizedBox::new().size(size).child(root)))
        .build()?;
    let window_id = runtime.window_ids()[0];
    Ok((runtime, window_id))
}

fn click(runtime: &mut Runtime, window_id: WindowId, point: Point) -> Result<()> {
    let mut down = PointerEvent::new(PointerEventKind::Down, point);
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    runtime.handle_event(window_id, Event::Pointer(down))?;
    let mut up = PointerEvent::new(PointerEventKind::Up, point);
    up.button = Some(PointerButton::Primary);
    runtime.handle_event(window_id, Event::Pointer(up))
}

fn center(rect: Rect) -> Point {
    Point::new(
        rect.x() + rect.width() * 0.5,
        rect.y() + rect.height() * 0.5,
    )
}

fn theme_segment(output: &RenderOutput, label: &str) -> Rect {
    let switch = node(
        &output.semantics,
        SemanticsRole::RadioGroup,
        WIDGET_BOOK_THEME_SWITCH_NAME,
    )
    .expect("theme switch present")
    .bounds;
    output
        .semantics
        .iter()
        .find(|node| {
            node.role == SemanticsRole::RadioButton
                && node.name.as_deref() == Some(label)
                && node.bounds.intersection(switch).is_some()
        })
        .unwrap_or_else(|| panic!("theme segment {label} present"))
        .bounds
}

fn story_and_category_titles() -> Vec<&'static str> {
    page_items()
        .iter()
        .filter_map(|item| match item {
            PageItem::Category(category) => Some(category.title()),
            PageItem::Story(story) => Some(story.title),
            PageItem::Intro | PageItem::NoResults => None,
        })
        .collect()
}

#[test]
fn registry_groups_stories_by_category_in_page_order() {
    let mut seen_ids = HashSet::new();
    let mut seen_titles = HashSet::new();
    let mut last_category = 0;
    for story in stories() {
        assert!(seen_ids.insert(story.id), "duplicate story id {}", story.id);
        assert!(
            seen_titles.insert(story.title),
            "duplicate story title {}",
            story.title
        );
        assert!(
            story
                .id
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
            "story id {} should be kebab case",
            story.id
        );
        assert!(!story.summary.is_empty() && story.summary.ends_with('.'));
        let category = Category::ALL
            .iter()
            .position(|category| *category == story.category)
            .expect("story category is registered");
        assert!(
            category >= last_category,
            "{} breaks category order",
            story.title
        );
        last_category = category;
    }
    for category in Category::ALL {
        assert!(
            stories().iter().any(|story| story.category == category),
            "{} has no stories",
            category.title()
        );
    }
    assert_eq!(story("button").map(|story| story.title), Some("Button"));
}

#[test]
fn page_items_follow_the_registry() {
    let items = page_items();
    assert!(matches!(items.first(), Some(PageItem::Intro)));
    assert!(matches!(items.last(), Some(PageItem::NoResults)));
    let mut expected = Vec::new();
    for category in Category::ALL {
        expected.push(category.title());
        expected.extend(
            stories()
                .iter()
                .filter(|story| story.category == category)
                .map(|story| story.title),
        );
    }
    assert_eq!(story_and_category_titles(), expected);
}

/// Every story renders a block in the theme named `theme_name`. A test per
/// theme, so the themes render in parallel.
fn every_story_renders_in(theme_name: &str) -> Result<()> {
    let theme = builtin_themes()
        .into_iter()
        .find(|(name, _)| *name == theme_name)
        .map(|(_, theme)| theme)
        .expect("a built-in theme");
    for story in stories() {
        let sections = (story.build)(&StoryCtx::new(theme));
        assert!(!sections.is_empty(), "{} has no specimens", story.title);
        let (mut runtime, window_id) =
            build_runtime(Size::new(1120.0, 1400.0), story_block(story, theme))?;
        let output = runtime.render(window_id)?;
        let region = named(&output.semantics, &story.region_name())
            .unwrap_or_else(|| panic!("{} renders its block in {theme_name}", story.title));
        assert!(
            region.bounds.width() > 0.0 && region.bounds.height() > 80.0,
            "{} block is empty in {theme_name}: {:?}",
            story.title,
            region.bounds
        );
    }
    Ok(())
}

#[test]
fn every_story_renders_in_light() -> Result<()> {
    every_story_renders_in("light")
}

#[test]
fn every_story_renders_in_dark() -> Result<()> {
    every_story_renders_in("dark")
}

#[test]
fn every_story_renders_in_neutral() -> Result<()> {
    every_story_renders_in("neutral")
}

#[test]
fn every_story_renders_in_neutral_dark() -> Result<()> {
    every_story_renders_in("neutral dark")
}

#[test]
fn every_story_renders_in_void() -> Result<()> {
    every_story_renders_in("void")
}

#[test]
fn rail_lists_every_category_and_story_in_page_order() -> Result<()> {
    let (mut runtime, window_id) =
        build_runtime(Size::new(1280.0, 800.0), build_widget_book_gallery())?;
    let output = runtime.render(window_id)?;
    assert!(named(&output.semantics, WIDGET_BOOK_NAV_NAME).is_some());
    let expected = story_and_category_titles();
    let links: Vec<&str> = output
        .semantics
        .iter()
        .filter(|node| node.role == SemanticsRole::Link)
        .filter_map(|node| node.name.as_deref())
        .filter(|name| expected.contains(name))
        .collect();
    assert_eq!(links, expected);
    Ok(())
}

#[test]
fn rail_link_jumps_to_its_story_and_marks_it_current() -> Result<()> {
    let app = open_book(Size::new(1280.0, 900.0))?;
    let window = app.main_window()?;
    window
        .get_by_role(SemanticsRole::Link)
        .with_name("Switch")
        .click()?;
    window.run_until_idle()?;

    let nodes = window.snapshot()?.accessibility.nodes;
    let gallery = gallery_bounds(&nodes);
    let region = named(&nodes, "Switch story").expect("the switch story is laid out");
    assert!(
        region.bounds.y() >= gallery.y() - 1.0 && region.bounds.y() <= gallery.y() + 48.0,
        "the story should land at the top of the page: story={:?} gallery={gallery:?}",
        region.bounds
    );
    assert_eq!(selected_rail_links(&nodes), vec!["Switch".to_string()]);
    Ok(())
}

#[test]
fn scrolling_the_page_moves_the_current_rail_entry_and_keeps_it_visible() -> Result<()> {
    let app = open_book(Size::new(1280.0, 720.0))?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);
    assert!(selected_rail_links(&window.snapshot()?.accessibility.nodes).is_empty());

    for _ in 0..60 {
        gallery.scroll_pixels(Vector::new(0.0, -600.0))?;
    }
    window.run_until_idle()?;

    let nodes = window.snapshot()?.accessibility.nodes;
    let current = selected_rail_links(&nodes);
    assert_eq!(
        current.len(),
        1,
        "exactly one rail entry is current: {current:?}"
    );
    let current_story = stories()
        .iter()
        .find(|story| story.title == current[0])
        .expect("the current entry is a story near the end of the page");
    let region =
        named(&nodes, &current_story.region_name()).expect("the current story is laid out");
    let viewport = gallery_bounds(&nodes);
    assert!(
        region.bounds.intersection(viewport).is_some(),
        "the current story is on screen"
    );
    let link = node(&nodes, SemanticsRole::Link, &current[0]).expect("current rail link");
    let rail = node(&nodes, SemanticsRole::ScrollView, RAIL_SCROLL_NAME).expect("rail present");
    assert!(
        link.bounds.y() >= rail.bounds.y() && link.bounds.max_y() <= rail.bounds.max_y(),
        "the rail scrolls to keep the current entry visible: link={:?} rail={:?}",
        link.bounds,
        rail.bounds
    );
    Ok(())
}

#[test]
fn filter_collapses_non_matching_stories_and_categories() -> Result<()> {
    let app = open_book(Size::new(1280.0, 900.0))?;
    let window = app.main_window()?;
    let search = window
        .get_by_role(SemanticsRole::TextInput)
        .with_name(WIDGET_BOOK_SEARCH_NAME);
    search.fill("progress")?;
    window.run_until_idle()?;

    let nodes = window.snapshot()?.accessibility.nodes;
    assert!(named(&nodes, "Progress bar story").is_some());
    assert!(named(&nodes, "Button story").is_none());
    assert!(node(&nodes, SemanticsRole::Link, "Progress bar").is_some());
    assert!(node(&nodes, SemanticsRole::Link, "Feedback and status").is_some());
    assert!(node(&nodes, SemanticsRole::Link, "Button").is_none());
    assert!(node(&nodes, SemanticsRole::Link, "Actions").is_none());

    search.fill("zzzz-no-such-widget")?;
    window.run_until_idle()?;
    let nodes = window.snapshot()?.accessibility.nodes;
    assert!(named(&nodes, "No components match your filter").is_some());
    let titles = story_and_category_titles();
    assert!(
        !nodes.iter().any(|node| node.role == SemanticsRole::Link
            && node
                .name
                .as_deref()
                .is_some_and(|name| titles.contains(&name))),
        "no rail links remain for an unmatched filter"
    );
    Ok(())
}

#[test]
fn theme_switch_rebuilds_stories_with_the_selected_theme() -> Result<()> {
    let (mut runtime, window_id) =
        build_runtime(Size::new(1280.0, 800.0), build_widget_book_gallery())?;
    let light = runtime.render(window_id)?;
    let dark = DefaultTheme::dark();
    assert!(solid_fill_colors(&light).contains(&DefaultTheme::sui().colors.neutrals.window));
    assert!(!solid_fill_colors(&light).contains(&dark.colors.neutrals.window));

    click(
        &mut runtime,
        window_id,
        center(theme_segment(&light, "Dark")),
    )?;
    runtime.handle_event(window_id, Event::Window(WindowEvent::RedrawRequested))?;
    let output = runtime.render(window_id)?;
    let fills = solid_fill_colors(&output);
    assert!(
        fills.contains(&dark.colors.neutrals.window),
        "the page follows the dark theme"
    );
    assert!(
        fills.contains(&dark.colors.neutrals.subtle),
        "story stages follow the dark theme"
    );
    assert!(
        fills.contains(&dark.palette.accent),
        "rebuilt specimens paint with the dark accent"
    );
    let switch = node(
        &output.semantics,
        SemanticsRole::RadioGroup,
        WIDGET_BOOK_THEME_SWITCH_NAME,
    )
    .expect("theme switch present");
    assert_eq!(switch.value, Some(SemanticsValue::Text("Dark".to_string())));
    Ok(())
}

#[test]
fn embedded_book_follows_the_application_theme_by_default() -> Result<()> {
    let theme = Rc::new(RefCell::new(DefaultTheme::dark()));
    let (mut runtime, window_id) = build_runtime(
        Size::new(1280.0, 800.0),
        build_widget_book_gallery_with_theme(mutable_theme_reader(Rc::clone(&theme))),
    )?;
    let output = runtime.render(window_id)?;
    let switch = node(
        &output.semantics,
        SemanticsRole::RadioGroup,
        WIDGET_BOOK_THEME_SWITCH_NAME,
    )
    .expect("theme switch present");
    assert_eq!(switch.value, Some(SemanticsValue::Text("App".to_string())));
    assert!(solid_fill_colors(&output).contains(&DefaultTheme::dark().colors.neutrals.window));

    // Hosts invalidate the window when their theme changes.
    *theme.borrow_mut() = DefaultTheme::neutral();
    runtime.handle_event(
        window_id,
        Event::Window(WindowEvent::Resized(Size::new(1280.0, 800.0))),
    )?;
    let output = runtime.render(window_id)?;
    assert!(
        solid_fill_colors(&output).contains(&DefaultTheme::neutral().colors.neutrals.window),
        "the App choice tracks application theme changes"
    );
    Ok(())
}

#[test]
fn embedded_book_repaints_when_the_theme_reader_changes() -> Result<()> {
    assert_widget_repaints_after_theme_change(
        WINDOW_TITLE,
        Size::new(1280.0, 760.0),
        build_widget_book_gallery_with_theme,
    )
}

#[test]
fn narrow_windows_hide_the_rail() -> Result<()> {
    let (mut runtime, window_id) =
        build_runtime(Size::new(720.0, 800.0), build_widget_book_gallery())?;
    let output = runtime.render(window_id)?;
    assert!(named(&output.semantics, WIDGET_BOOK_NAV_NAME).is_none());
    let gallery = gallery_bounds(&output.semantics);
    assert!(gallery.x() <= 1.0 && gallery.width() >= 719.0);
    let search = node(
        &output.semantics,
        SemanticsRole::TextInput,
        WIDGET_BOOK_SEARCH_NAME,
    )
    .expect("the filter stays available");
    let switch = node(
        &output.semantics,
        SemanticsRole::ComboBox,
        WIDGET_BOOK_THEME_SWITCH_NAME,
    )
    .expect("the theme switch collapses to a select when the segments do not fit");
    assert!(search.bounds.max_x() <= switch.bounds.x());
    assert!(switch.bounds.max_x() <= 720.0);
    assert_eq!(
        switch.value,
        Some(SemanticsValue::Text("Light".to_string()))
    );
    Ok(())
}

#[test]
fn widget_book_demo_text_roles_use_semantic_theme_weights() {
    let theme = DefaultTheme::default();
    assert_eq!(
        DemoTextRole::PageTitle.weight(theme).value(),
        theme.font_weights.semibold
    );
    assert_eq!(
        DemoTextRole::SectionTitle.weight(theme).value(),
        theme.font_weights.semibold
    );
    assert_eq!(
        DemoTextRole::CardTitle.weight(theme).value(),
        theme.font_weights.medium
    );
    assert_eq!(
        DemoTextRole::Body.weight(theme).value(),
        theme.font_weights.normal
    );
}

#[test]
fn widget_book_gallery_wheel_scroll_updates_screenshot() -> Result<()> {
    let app = open_book(Size::new(1280.0, 720.0))?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);

    let before = gallery.capture_screenshot()?;
    gallery.scroll_pixels(Vector::new(0.0, -12.0))?;
    let small = gallery.capture_screenshot()?;
    gallery.scroll_pixels(Vector::new(0.0, -360.0))?;
    let large = gallery.capture_screenshot()?;

    assert_ne!(before, small);
    assert_ne!(small, large);
    Ok(())
}

#[test]
fn widget_book_gallery_scroll_bar_drag_repaints_content_immediately() -> Result<()> {
    let app = open_book(Size::new(1280.0, 720.0))?;
    let window = app.main_window()?;
    let gallery = window
        .get_by_role(SemanticsRole::ScrollView)
        .with_name(GALLERY_SCROLL_NAME);
    let snapshot = window.snapshot()?;
    let scroll_bar = node(
        &snapshot.accessibility.nodes,
        SemanticsRole::Slider,
        GALLERY_SCROLL_BAR_NAME,
    )
    .expect("widget book gallery scroll bar should be present");
    let before_value = scroll_bar.value.clone();
    let start = Point::new(
        scroll_bar.bounds.x() + scroll_bar.bounds.width() * 0.5,
        scroll_bar.bounds.y() + 24.0,
    );
    let end = Point::new(
        start.x,
        (start.y + 300.0).min(scroll_bar.bounds.max_y() - 24.0),
    );

    let root = window.root();
    let mut down = PointerEvent::new(PointerEventKind::Down, start);
    down.pointer_id = 73;
    down.button = Some(PointerButton::Primary);
    down.buttons = PointerButtons::new(1);
    root.dispatch_event(Event::Pointer(down))?;

    let before = gallery.capture_screenshot()?;
    let content_crop = Rect::new(
        16.0,
        16.0,
        (before.width() as f32 - 64.0).max(1.0),
        (before.height() as f32 - 32.0).max(1.0),
    );
    let before_content = before.crop(content_crop)?;

    let mut moved = PointerEvent::new(PointerEventKind::Move, end);
    moved.pointer_id = 73;
    moved.buttons = PointerButtons::new(1);
    moved.delta = end - start;
    root.dispatch_event(Event::Pointer(moved))?;

    let after_value = node(
        &window.snapshot()?.accessibility.nodes,
        SemanticsRole::Slider,
        GALLERY_SCROLL_BAR_NAME,
    )
    .and_then(|node| node.value.clone());
    let after_content = gallery.capture_screenshot()?.crop(content_crop)?;

    assert_ne!(before_value, after_value, "the drag should move the thumb");
    assert_ne!(
        before_content, after_content,
        "one captured mouse move must redraw gallery content, not only the thumb"
    );
    Ok(())
}

#[test]
fn widget_book_gallery_exposes_visible_scroll_bar() -> Result<()> {
    let (mut runtime, window_id) =
        build_runtime(Size::new(1280.0, 720.0), build_widget_book_gallery())?;
    let output = runtime.render(window_id)?;
    let gallery = gallery_bounds(&output.semantics);
    let scroll_bar = node(
        &output.semantics,
        SemanticsRole::Slider,
        GALLERY_SCROLL_BAR_NAME,
    )
    .expect("widget book gallery scroll bar should be present");

    assert!(scroll_bar.bounds.x() >= gallery.x());
    assert!(scroll_bar.bounds.max_x() <= gallery.max_x());
    assert!(scroll_bar.bounds.y() >= gallery.y());
    assert!(scroll_bar.bounds.max_y() <= gallery.max_y());
    assert!(scroll_bar.bounds.height() >= gallery.height() - 8.0);
    Ok(())
}

#[test]
fn widget_book_gallery_scroll_bar_uses_themed_metrics() {
    let theme = DefaultTheme::touch();
    let output = render_widget_with_size(
        WINDOW_TITLE,
        Size::new(1280.0, 420.0),
        build_widget_book_gallery_with_theme(theme_reader(theme)),
    );
    let scroll_bar = node(
        &output.semantics,
        SemanticsRole::Slider,
        GALLERY_SCROLL_BAR_NAME,
    )
    .expect("widget book gallery scroll bar should be present");

    assert_eq!(
        scroll_bar.bounds.width(),
        theme.metrics.scroll_bar_thickness
    );
}

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "slow; run `cargo run -p sinomo-ui-demo --bin sui-demo-artifacts` to generate artifacts"]
fn widget_book_generates_visual_artifacts() -> Result<()> {
    let root = unique_visual_artifact_test_dir("all");
    super::visual_artifacts::write_visual_artifacts_to(&root)?;
    for story in stories() {
        for theme in super::visual_artifacts::CAPTURE_THEMES {
            let path = root
                .join("stories")
                .join(story.id)
                .join(format!("{}.png", theme.to_lowercase()));
            assert!(path.exists(), "missing {}", path.display());
        }
    }
    assert!(root.join("overview-light").join("window.png").exists());
    assert!(
        root.join("hdr-validation")
            .join("hdr-intermediate.exr")
            .exists()
    );
    std::fs::remove_dir_all(&root).ok();
    Ok(())
}

#[cfg(feature = "artifacts")]
#[test]
#[ignore = "diagnostic benchmark for current headless widget-book scroll status"]
fn widget_book_headless_scroll_current_status_benchmark() -> Result<()> {
    let _guard = headless_benchmark_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let app = TestApp::from_runtime(build_widget_book_application().build()?)?;
    let window = app.main_window()?;
    set_detailed_scene_statistics_mode(&window)?;
    let samples = collect_headless_scroll_benchmark_samples(&window, GALLERY_SCROLL_NAME, 24)?;
    print_headless_benchmark_summary("Widget Book Headless Scroll Benchmark", &samples);
    Ok(())
}

#[test]
#[ignore = "diagnostic benchmark for widget-book tree construction and destruction"]
fn widget_book_tree_creation_current_status_benchmark() {
    let mut creation_ms = Vec::with_capacity(7);
    let mut destruction_ms = Vec::with_capacity(7);
    for _ in 0..7 {
        let started = Instant::now();
        let gallery = std::hint::black_box(build_widget_book_gallery());
        creation_ms.push(started.elapsed().as_secs_f64() * 1_000.0);

        let started = Instant::now();
        drop(gallery);
        destruction_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    creation_ms.sort_by(f64::total_cmp);
    destruction_ms.sort_by(f64::total_cmp);
    println!(
        "widget-book tree creation: median={:.3} ms min={:.3} ms max={:.3} ms",
        creation_ms[creation_ms.len() / 2],
        creation_ms[0],
        creation_ms[creation_ms.len() - 1]
    );
    println!(
        "widget-book tree destruction: median={:.3} ms min={:.3} ms max={:.3} ms",
        destruction_ms[destruction_ms.len() / 2],
        destruction_ms[0],
        destruction_ms[destruction_ms.len() - 1]
    );
}
