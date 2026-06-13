//! # gpui_squircle
//!
//! A squircle component for [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).
//!
//! Squircles are rounded rectangles with smooth, continuous curvature that
//! look more natural than standard CSS-style rounded corners. They are
//! commonly used in Apple's design language.
//!
//! ## Usage
//!
//! ```ignore
//! use gpui_squircle::{squircle, SquircleStyled};
//!
//! squircle()
//!     .rounded(px(50.))
//!     .bg(gpui::red())
//!     .absolute_expand()
//! ```

use figma_squircle::{FigmaSquircleParams, get_svg_path};
use gpui::{
    AnyElement, App, Background, Bounds, CornersRefinement, Element, ElementId, GlobalElementId,
    Hitbox, InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId,
    ParentElement, PathBuilder, Pixels, Refineable, Size, StatefulInteractiveElement, Style,
    StyleRefinement, Styled, Window, point, px,
};
use lyon::{
    extra::parser::{ParserOptions, PathParser, Source},
    path::Path as LyonPath,
};
use smallvec::SmallVec;

mod style;
pub use style::{SquircleStyleRefinement, Styled as SquircleStyled};

/// Internal options for building and painting squircle paths.
struct BuildAndPaintOptions {
    builder: PathBuilder,
    background: Background,
}

impl BuildAndPaintOptions {
    fn fill(background: Background) -> Self {
        Self {
            builder: PathBuilder::fill(),
            background,
        }
    }

    fn stroke(background: Background, border_width: f32) -> Self {
        Self {
            builder: PathBuilder::stroke(px(border_width)),
            background,
        }
    }
}

/// Determines how the border is positioned relative to the squircle's bounds.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub enum BorderMode {
    /// Border is centered on the edge (default behavior).
    #[default]
    Center,
    /// Border is drawn outside the squircle's bounds.
    Outside,
    /// Border is drawn inside the squircle's bounds.
    Inside,
}

/// A squircle element for GPUI.
///
/// Squircles provide smooth, continuous corner curvature unlike standard
/// rounded rectangles. Use [`squircle()`] to create a new instance.
pub struct Squircle {
    pub style: SquircleStyleRefinement,
    interactivity: Interactivity,
    children: SmallVec<[AnyElement; 2]>,
}

impl SquircleStyled for Squircle {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style.inner
    }

    fn outer_style(&mut self) -> &mut SquircleStyleRefinement {
        &mut self.style
    }
}

impl Squircle {
    fn new() -> Self {
        Self {
            style: SquircleStyleRefinement::default(),
            interactivity: Interactivity::new(),
            children: SmallVec::new(),
        }
    }

    /// Makes the squircle fill its parent element completely.
    ///
    /// This sets absolute positioning with all edges at zero, making it
    /// useful as a background layer that ignores parent padding.
    pub fn absolute_expand(mut self) -> Self {
        self.style.inner = self
            .style
            .inner
            .size_full()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .right_0();

        self
    }
}

/// Calculates size and border offsets based on the border mode.
#[inline(always)]
fn get_size_and_border_offsets(border_mode: BorderMode, border_width: f32) -> (f32, f32) {
    match border_mode {
        BorderMode::Outside => (-border_width, border_width - 2.),
        BorderMode::Inside => (border_width, -(border_width - 1.)),
        BorderMode::Center => (0., 0.),
    }
}

/// Builds a squircle Lyon path from pre-extracted style values.
fn build_squircle_path(
    size: Size<Pixels>,
    border_offset: f32,
    corner_radii: &CornersRefinement<Pixels>,
    corner_smoothing: f32,
    preserve_smoothing: bool,
) -> Option<LyonPath> {
    let params = FigmaSquircleParams {
        width: size.width.to_f64() as f32,
        height: size.height.to_f64() as f32,
        corner_radius: None,
        top_left_corner_radius: Some(
            corner_radii.top_left.unwrap_or_default().to_f64() as f32 + border_offset,
        ),
        top_right_corner_radius: Some(
            corner_radii.top_right.unwrap_or_default().to_f64() as f32 + border_offset,
        ),
        bottom_left_corner_radius: Some(
            corner_radii.bottom_left.unwrap_or_default().to_f64() as f32 + border_offset,
        ),
        bottom_right_corner_radius: Some(
            corner_radii.bottom_right.unwrap_or_default().to_f64() as f32 + border_offset,
        ),
        corner_smoothing,
        preserve_smoothing,
    };
    let svg_path = get_svg_path(params);
    let mut lyon_builder = LyonPath::builder();
    let parsed = PathParser::new().parse(
        &ParserOptions::DEFAULT,
        &mut Source::new(svg_path.chars()),
        &mut lyon_builder,
    );
    if parsed.is_err() {
        return None;
    }
    Some(lyon_builder.build())
}

/// Paints squircle paths using pre-extracted style values (no `&self` borrow needed).
fn paint_squircle<const N: usize>(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    size_offset: f32,
    border_offset: f32,
    corner_radii: &CornersRefinement<Pixels>,
    corner_smoothing: f32,
    preserve_smoothing: bool,
    options: [BuildAndPaintOptions; N],
) {
    let size_offset_px = px(size_offset);
    let size = bounds.size - gpui::size(size_offset_px, size_offset_px);

    let Some(path) = build_squircle_path(
        size,
        border_offset,
        corner_radii,
        corner_smoothing,
        preserve_smoothing,
    ) else {
        return;
    };

    let (origin_x, origin_y) = (
        (bounds.origin.x + size_offset_px / 2.).to_f64() as f32,
        (bounds.origin.y + size_offset_px / 2.).to_f64() as f32,
    );

    let mut opts = options;
    for event in path.iter() {
        match event {
            lyon::path::Event::Begin { at } => {
                let at = point(px(origin_x + at.x), px(origin_y + at.y));
                for o in opts.as_mut() {
                    o.builder.move_to(at);
                }
            }
            lyon::path::Event::Line { from: _, to } => {
                let to = point(px(origin_x + to.x), px(origin_y + to.y));
                for o in opts.as_mut() {
                    o.builder.line_to(to);
                }
            }
            lyon::path::Event::Quadratic { from: _, ctrl, to } => {
                let to = point(px(origin_x + to.x), px(origin_y + to.y));
                let ctrl = point(px(origin_x + ctrl.x), px(origin_y + ctrl.y));
                for o in opts.as_mut() {
                    o.builder.curve_to(to, ctrl);
                }
            }
            lyon::path::Event::Cubic {
                from: _,
                ctrl1,
                ctrl2,
                to,
            } => {
                let to = point(px(origin_x + to.x), px(origin_y + to.y));
                let ctrl1 = point(px(origin_x + ctrl1.x), px(origin_y + ctrl1.y));
                let ctrl2 = point(px(origin_x + ctrl2.x), px(origin_y + ctrl2.y));
                for o in opts.as_mut() {
                    o.builder.cubic_bezier_to(to, ctrl1, ctrl2);
                }
            }
            lyon::path::Event::End { close, .. } => {
                if close {
                    for o in opts.as_mut() {
                        o.builder.close();
                    }
                }
            }
        }
    }
    for BuildAndPaintOptions {
        builder,
        background,
        ..
    } in opts
    {
        if let Ok(path) = builder.build() {
            window.paint_path(path, background);
        }
    }
}

/// State passed between request_layout, prepaint, and paint.
pub struct SquircleLayoutState {
    style: Style,
    _child_layout_ids: SmallVec<[LayoutId; 4]>,
}

impl Element for Squircle {
    type RequestLayoutState = SquircleLayoutState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.interactivity.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.interactivity.source_location()
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style.inner);

        // The interactivity needs the base_style for pseudo-class merging.
        self.interactivity.base_style = Box::new(self.style.inner.clone());

        // Request layouts for all children and collect their layout IDs
        let child_layout_ids: SmallVec<[LayoutId; 4]> = self
            .children
            .iter_mut()
            .map(|child| child.request_layout(window, cx))
            .collect();

        let layout_id = window.request_layout(style.clone(), child_layout_ids.iter().copied(), cx);
        (
            layout_id,
            SquircleLayoutState {
                style,
                _child_layout_ids: child_layout_ids,
            },
        )
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Hitbox> {
        self.interactivity.prepaint(
            global_id,
            inspector_id,
            bounds,
            bounds.size,
            window,
            cx,
            |_style, _scroll_offset, hitbox, window, cx| {
                for child in &mut self.children {
                    child.prepaint(window, cx);
                }
                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = &mut request_layout.style;
        style.refine(&self.style.inner);

        // Determine the effective background, taking hover/active state into account
        let hovered = prepaint
            .as_ref()
            .is_some_and(|hitbox| hitbox.is_hovered(window));
        let active = self.interactivity.active.unwrap_or(false);
        let background = if active {
            self.style.active_bg.or(self.style.background)
        } else if hovered {
            self.style.hover_bg.or(self.style.background)
        } else {
            self.style.background
        };

        let border_color = self.style.border_color;
        let border_width = self.style.border_width.unwrap_or_default().to_f64() as f32;
        let border_mode = self.style.border_mode.unwrap_or_default();
        let corner_radii = self.style.corner_radii.clone();
        let corner_smoothing = self.style.corner_smoothing.unwrap_or(px(1.)).to_f64() as f32;
        let preserve_smoothing = self.style.preserve_smoothing.unwrap_or(true);

        style.paint(bounds, window, cx, |window, cx| {
            match (background, border_color) {
                (Some(bg), None) => {
                    paint_squircle(
                        window,
                        bounds,
                        0.,
                        0.,
                        &corner_radii,
                        corner_smoothing,
                        preserve_smoothing,
                        [BuildAndPaintOptions::fill(bg)],
                    );
                }

                (Some(bg), Some(border_c)) => {
                    let (size_offset, border_offset) =
                        get_size_and_border_offsets(border_mode, border_width);

                    if size_offset == 0. {
                        paint_squircle(
                            window,
                            bounds,
                            0.,
                            border_offset,
                            &corner_radii,
                            corner_smoothing,
                            preserve_smoothing,
                            [
                                BuildAndPaintOptions::fill(bg),
                                BuildAndPaintOptions::stroke(border_c, border_width),
                            ],
                        );
                    } else {
                        paint_squircle(
                            window,
                            bounds,
                            0.,
                            0.,
                            &corner_radii,
                            corner_smoothing,
                            preserve_smoothing,
                            [BuildAndPaintOptions::fill(bg)],
                        );

                        paint_squircle(
                            window,
                            bounds,
                            size_offset,
                            border_offset,
                            &corner_radii,
                            corner_smoothing,
                            preserve_smoothing,
                            [BuildAndPaintOptions::stroke(border_c, border_width)],
                        );
                    }
                }

                (None, None) => (),

                (None, Some(border_c)) => {
                    let (size_offset, border_offset) =
                        get_size_and_border_offsets(border_mode, border_width);

                    paint_squircle(
                        window,
                        bounds,
                        size_offset,
                        border_offset,
                        &corner_radii,
                        corner_smoothing,
                        preserve_smoothing,
                        [BuildAndPaintOptions::stroke(border_c, border_width)],
                    );
                }
            }

            // Paint children on top
            for child in &mut self.children {
                child.paint(window, cx);
            }
        });
    }
}

impl IntoElement for Squircle {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl InteractiveElement for Squircle {
    fn interactivity(&mut self) -> &mut Interactivity {
        &mut self.interactivity
    }
}

impl StatefulInteractiveElement for Squircle {}

impl ParentElement for Squircle {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// Creates a new [`Squircle`] element.
///
/// # Example
///
/// ```ignore
/// use gpui_squircle::{squircle, SquircleStyled};
/// use gpui::px;
///
/// squircle()
///     .rounded(px(20.))
///     .bg(gpui::blue())
///     .size(px(100.))
/// ```
pub fn squircle() -> Squircle {
    Squircle::new()
}
