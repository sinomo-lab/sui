use std::collections::{HashMap, HashSet};

use sui_core::{Point, WidgetId};

use crate::{CommandKey, widget::FocusRequest};

const MAX_OVERLAY_TRACE_SAMPLES: usize = 512;

/// Broad behavior category used by diagnostics and accessibility adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    Tooltip,
    Popover,
    Menu,
    Dialog,
    Sheet,
    CommandPalette,
    Notification,
    DragPreview,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OverlayModality {
    #[default]
    Modeless,
    Modal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OverlayInitialFocus {
    #[default]
    None,
    Owner,
    FirstFocusable,
}

/// Focus behavior shared by every transient presentation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayFocusBehavior {
    pub initial: OverlayInitialFocus,
    pub trap: bool,
    pub restore: bool,
}

impl OverlayFocusBehavior {
    pub const NONE: Self = Self {
        initial: OverlayInitialFocus::None,
        trap: false,
        restore: false,
    };

    pub const RESTORE: Self = Self {
        initial: OverlayInitialFocus::None,
        trap: false,
        restore: true,
    };

    pub const CONTAINED: Self = Self {
        initial: OverlayInitialFocus::FirstFocusable,
        trap: true,
        restore: true,
    };
}

impl Default for OverlayFocusBehavior {
    fn default() -> Self {
        Self::RESTORE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayDismissPolicy {
    pub escape: bool,
    pub outside_pointer: bool,
}

impl OverlayDismissPolicy {
    pub const NONE: Self = Self {
        escape: false,
        outside_pointer: false,
    };

    pub const TRANSIENT: Self = Self {
        escape: true,
        outside_pointer: true,
    };
}

impl Default for OverlayDismissPolicy {
    fn default() -> Self {
        Self::TRANSIENT
    }
}

/// Runtime behavior declared by the logical owner of an overlay.
///
/// Overlay content remains an ordinary retained widget subtree. The runtime
/// uses this declaration to coordinate that subtree with every other overlay
/// in the same presentation root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayOptions {
    pub kind: OverlayKind,
    pub modality: OverlayModality,
    pub dismiss: OverlayDismissPolicy,
    pub focus: OverlayFocusBehavior,
}

impl OverlayOptions {
    pub const fn new(kind: OverlayKind) -> Self {
        Self {
            kind,
            modality: OverlayModality::Modeless,
            dismiss: OverlayDismissPolicy::TRANSIENT,
            focus: OverlayFocusBehavior::RESTORE,
        }
    }

    pub const fn modal(mut self, modal: bool) -> Self {
        self.modality = if modal {
            OverlayModality::Modal
        } else {
            OverlayModality::Modeless
        };
        self
    }

    pub const fn dismiss(mut self, dismiss: OverlayDismissPolicy) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub const fn focus(mut self, focus: OverlayFocusBehavior) -> Self {
        self.focus = focus;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayDismissReason {
    Escape,
    OutsidePointer,
    Programmatic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayDismissRequest {
    pub reason: OverlayDismissReason,
    pub pointer_position: Option<Point>,
}

impl OverlayDismissRequest {
    pub const fn new(reason: OverlayDismissReason) -> Self {
        Self {
            reason,
            pointer_position: None,
        }
    }

    pub const fn at(mut self, position: Point) -> Self {
        self.pointer_position = Some(position);
        self
    }
}

/// Typed runtime-to-widget request used for centralized dismissal arbitration.
pub const OVERLAY_DISMISS_REQUEST: CommandKey<OverlayDismissRequest> =
    CommandKey::new("sui.overlay.dismiss");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlaySnapshot {
    pub owner: WidgetId,
    pub parent: Option<WidgetId>,
    pub surfaces: Vec<WidgetId>,
    pub options: OverlayOptions,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverlayManagerSnapshot {
    pub overlays: Vec<OverlaySnapshot>,
    pub active_modal: Option<WidgetId>,
    pub focus_trap: Option<WidgetId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayTraceKind {
    Opened,
    Closed,
    Reordered,
    DismissRequested,
    FocusEntered,
    FocusRestored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayTraceSample {
    pub owner: WidgetId,
    pub kind: OverlayTraceKind,
    pub overlay_kind: OverlayKind,
    pub reason: Option<OverlayDismissReason>,
}

#[derive(Default)]
pub(crate) struct OverlayManager {
    active: Vec<OverlaySnapshot>,
    focus_origins: HashMap<WidgetId, Option<WidgetId>>,
    traces: Vec<OverlayTraceSample>,
}

impl OverlayManager {
    pub(crate) fn sync(
        &mut self,
        next: Vec<OverlaySnapshot>,
        graph: &crate::WidgetGraph,
        focused: Option<WidgetId>,
    ) -> Option<FocusRequest> {
        let previous_by_owner = self
            .active
            .iter()
            .map(|entry| (entry.owner, entry.clone()))
            .collect::<HashMap<_, _>>();
        let next_owners = next.iter().map(|entry| entry.owner).collect::<HashSet<_>>();

        let mut focus_request = None;
        for closed in self.active.iter().rev() {
            if next_owners.contains(&closed.owner) {
                continue;
            }
            push_trace(
                &mut self.traces,
                OverlayTraceSample {
                    owner: closed.owner,
                    kind: OverlayTraceKind::Closed,
                    overlay_kind: closed.options.kind,
                    reason: None,
                },
            );
            let origin = self.focus_origins.remove(&closed.owner).flatten();
            let focus_still_owned = focused.is_none_or(|focused| {
                !graph.contains(focused)
                    || focused == closed.owner
                    || graph.is_ancestor_of(closed.owner, focused)
            });
            if focus_request.is_none()
                && closed.options.focus.restore
                && focus_still_owned
                && let Some(origin) = origin.filter(|origin| graph.contains(*origin))
            {
                focus_request = Some(FocusRequest::Focus(origin));
                push_trace(
                    &mut self.traces,
                    OverlayTraceSample {
                        owner: closed.owner,
                        kind: OverlayTraceKind::FocusRestored,
                        overlay_kind: closed.options.kind,
                        reason: None,
                    },
                );
            }
        }

        for opened in &next {
            if previous_by_owner.contains_key(&opened.owner) {
                continue;
            }
            self.focus_origins.insert(opened.owner, focused);
            push_trace(
                &mut self.traces,
                OverlayTraceSample {
                    owner: opened.owner,
                    kind: OverlayTraceKind::Opened,
                    overlay_kind: opened.options.kind,
                    reason: None,
                },
            );
        }

        for opened in next.iter().rev() {
            if previous_by_owner.contains_key(&opened.owner)
                || focused.is_some_and(|focused| {
                    focused == opened.owner || graph.is_ancestor_of(opened.owner, focused)
                })
            {
                continue;
            }
            let initial = match opened.options.focus.initial {
                OverlayInitialFocus::None => None,
                OverlayInitialFocus::Owner => graph
                    .node(opened.owner)
                    .filter(|node| node.accepts_focus)
                    .map(|_| opened.owner)
                    .or_else(|| graph.first_focusable_within(opened.owner, false)),
                OverlayInitialFocus::FirstFocusable => graph
                    .first_focusable_within(opened.owner, false)
                    .or_else(|| {
                        graph
                            .node(opened.owner)
                            .filter(|node| node.accepts_focus)
                            .map(|_| opened.owner)
                    }),
            };
            if let Some(initial) = initial {
                focus_request = Some(FocusRequest::Focus(initial));
                push_trace(
                    &mut self.traces,
                    OverlayTraceSample {
                        owner: opened.owner,
                        kind: OverlayTraceKind::FocusEntered,
                        overlay_kind: opened.options.kind,
                        reason: None,
                    },
                );
                break;
            }
        }

        if self
            .active
            .iter()
            .map(|entry| entry.owner)
            .collect::<Vec<_>>()
            != next.iter().map(|entry| entry.owner).collect::<Vec<_>>()
            && !self.active.is_empty()
            && !next.is_empty()
            && let Some(top) = next.last()
        {
            push_trace(
                &mut self.traces,
                OverlayTraceSample {
                    owner: top.owner,
                    kind: OverlayTraceKind::Reordered,
                    overlay_kind: top.options.kind,
                    reason: None,
                },
            );
        }

        self.active = next;
        focus_request
    }

    pub(crate) fn snapshot(&self) -> OverlayManagerSnapshot {
        OverlayManagerSnapshot {
            overlays: self.active.clone(),
            active_modal: self
                .active
                .iter()
                .rev()
                .find(|entry| entry.options.modality == OverlayModality::Modal)
                .map(|entry| entry.owner),
            focus_trap: self
                .active
                .iter()
                .rev()
                .find(|entry| entry.options.focus.trap)
                .map(|entry| entry.owner),
        }
    }

    pub(crate) fn focus_trap(&self) -> Option<WidgetId> {
        self.active
            .iter()
            .rev()
            .find(|entry| entry.options.focus.trap)
            .map(|entry| entry.owner)
    }

    pub(crate) fn active_modal(&self) -> Option<WidgetId> {
        self.active
            .iter()
            .rev()
            .find(|entry| entry.options.modality == OverlayModality::Modal)
            .map(|entry| entry.owner)
    }

    pub(crate) fn topmost_dismissible(
        &self,
        reason: OverlayDismissReason,
    ) -> Option<&OverlaySnapshot> {
        self.active.iter().rev().find(|entry| match reason {
            OverlayDismissReason::Escape => entry.options.dismiss.escape,
            OverlayDismissReason::OutsidePointer => entry.options.dismiss.outside_pointer,
            OverlayDismissReason::Programmatic => true,
        })
    }

    pub(crate) fn trace_dismissal(
        &mut self,
        owner: WidgetId,
        kind: OverlayKind,
        reason: OverlayDismissReason,
    ) {
        push_trace(
            &mut self.traces,
            OverlayTraceSample {
                owner,
                kind: OverlayTraceKind::DismissRequested,
                overlay_kind: kind,
                reason: Some(reason),
            },
        );
    }

    pub(crate) fn take_traces(&mut self) -> Vec<OverlayTraceSample> {
        std::mem::take(&mut self.traces)
    }
}

fn push_trace(traces: &mut Vec<OverlayTraceSample>, sample: OverlayTraceSample) {
    if traces.len() == MAX_OVERLAY_TRACE_SAMPLES {
        traces.remove(0);
    }
    traces.push(sample);
}
