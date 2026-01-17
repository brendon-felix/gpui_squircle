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
    App, Background, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, Interactivity, IntoElement, LayoutId, PathBuilder, Pixels, Refineable,
    Size, StatefulInteractiveElement, Style, StyleRefinement, Styled, Window, point, px,
};
use lyon::{
    extra::parser::{ParserOptions, PathParser, Source},
    path::Path as LyonPath,
};

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

    /// Converts the squircle's style into parameters for the figma_squircle library.
    fn to_params(&self, width: f32, height: f32, border_offset: f32) -> FigmaSquircleParams {
        let style = &self.style;
        let corner_radii = &self.style.corner_radii;

        FigmaSquircleParams {
            width,
            height,
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
            corner_smoothing: style.corner_smoothing.unwrap_or(px(1.)).to_f64() as f32,
            preserve_smoothing: style.preserve_smoothing.unwrap_or(true),
        }
    }

    /// Builds a Lyon path from the squircle's SVG representation.
    fn build_lyon_path(&self, size: Size<Pixels>, border_offset: f32) -> Option<LyonPath> {
        let svg_path = get_svg_path(self.to_params(
            size.width.to_f64() as f32,
            size.height.to_f64() as f32,
            border_offset,
        ));

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

    /// Builds path(s) from the Lyon representation and paints them to the window.
    fn build_and_paint_paths<const N: usize>(
        &self,
        window: &mut Window,
        Bounds { origin, size }: Bounds<Pixels>,
        size_offset: f32,
        border_offset: f32,
        mut options: [BuildAndPaintOptions; N],
    ) {
        let size_offset_px = px(size_offset);
        let size = size - gpui::size(size_offset_px, size_offset_px);

        // If the path doesn't exist then the svg is malformed.
        // TODO: fallback to regular rounded rectangle if this case is met.
        let Some(path) = self.build_lyon_path(size, border_offset) else {
            return;
        };

        let (origin_x, origin_y) = (
            (origin.x + size_offset_px / 2.).to_f64() as f32,
            (origin.y + size_offset_px / 2.).to_f64() as f32,
        );

        // Convert Lyon path events to GPUI path commands
        for event in path.iter() {
            match event {
                lyon::path::Event::Begin { at } => {
                    let at = point(px(origin_x + at.x), px(origin_y + at.y));

                    for BuildAndPaintOptions { builder, .. } in options.as_mut() {
                        builder.move_to(at)
                    }
                }

                lyon::path::Event::Line { from: _, to } => {
                    let to = point(px(origin_x + to.x), px(origin_y + to.y));

                    for BuildAndPaintOptions { builder, .. } in options.as_mut() {
                        builder.line_to(to)
                    }
                }

                lyon::path::Event::Quadratic { from: _, ctrl, to } => {
                    let to = point(px(origin_x + to.x), px(origin_y + to.y));
                    let ctrl = point(px(origin_x + ctrl.x), px(origin_y + ctrl.y));

                    for BuildAndPaintOptions { builder, .. } in options.as_mut() {
                        builder.curve_to(to, ctrl);
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

                    for BuildAndPaintOptions { builder, .. } in options.as_mut() {
                        builder.cubic_bezier_to(to, ctrl1, ctrl2)
                    }
                }

                lyon::path::Event::End { close, .. } => {
                    if close {
                        for BuildAndPaintOptions { builder, .. } in options.as_mut() {
                            builder.close()
                        }
                    }
                }
            }
        }

        for BuildAndPaintOptions {
            builder,
            background,
            ..
        } in options
        {
            let Ok(path) = builder.build() else { continue };
            window.paint_path(path, background);
        }
    }

    /// Calculates size and border offsets based on the current border mode.
    #[inline(always)]
    fn get_size_and_border_offsets(&self, border_width: f32) -> (f32, f32) {
        match self.style.border_mode.unwrap_or_default() {
            BorderMode::Outside => (-border_width, border_width - 2.),
            BorderMode::Inside => (border_width, -(border_width - 1.)),
            BorderMode::Center => (0., 0.),
        }
    }
}

impl Element for Squircle {
    type RequestLayoutState = Style;
    type PrepaintState = Option<()>;

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

        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<()> {
        None
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let style_refinement = &self.style;

        style.refine(&style_refinement.inner);

        style.paint(bounds, window, cx, |window, _cx| {
            match (style_refinement.background, style_refinement.border_color) {
                (Some(bg), None) => {
                    self.build_and_paint_paths(
                        window,
                        bounds,
                        0.,
                        0.,
                        [BuildAndPaintOptions::fill(bg)],
                    );
                }

                (Some(bg), Some(border_color)) => {
                    let border_width =
                        style_refinement.border_width.unwrap_or_default().to_f64() as f32;
                    let (size_offset, border_offset) =
                        self.get_size_and_border_offsets(border_width);

                    if size_offset == 0. {
                        // We can generate the same path for both the fill and the stroke.
                        self.build_and_paint_paths(
                            window,
                            bounds,
                            0.,
                            border_offset,
                            [
                                BuildAndPaintOptions::fill(bg),
                                BuildAndPaintOptions::stroke(border_color, border_width),
                            ],
                        );
                    } else {
                        // We need to generate different paths for the fill and
                        // stroke as they have different corner radii and sizes.
                        self.build_and_paint_paths(
                            window,
                            bounds,
                            0.,
                            0.,
                            [BuildAndPaintOptions::fill(bg)],
                        );

                        self.build_and_paint_paths(
                            window,
                            bounds,
                            size_offset,
                            border_offset,
                            [BuildAndPaintOptions::stroke(border_color, border_width)],
                        );
                    }
                }

                (None, None) => (),

                (None, Some(border_color)) => {
                    let border_width =
                        style_refinement.border_width.unwrap_or_default().to_f64() as f32;

                    let (size_offset, border_offset) =
                        self.get_size_and_border_offsets(border_width);

                    self.build_and_paint_paths(
                        window,
                        bounds,
                        size_offset,
                        border_offset,
                        [BuildAndPaintOptions::stroke(border_color, border_width)],
                    );
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn test_squircle_creation() {
        let sq = squircle();
        assert!(sq.style.background.is_none());
        assert!(sq.style.border_color.is_none());
        assert!(sq.style.border_width.is_none());
        assert!(sq.style.border_mode.is_none());
        assert!(sq.style.corner_smoothing.is_none());
        assert!(sq.style.preserve_smoothing.is_none());
    }

    #[test]
    fn test_border_mode_default() {
        assert_eq!(BorderMode::default(), BorderMode::Center);
    }

    #[test]
    fn test_border_mode_equality() {
        assert_eq!(BorderMode::Center, BorderMode::Center);
        assert_eq!(BorderMode::Inside, BorderMode::Inside);
        assert_eq!(BorderMode::Outside, BorderMode::Outside);
        assert_ne!(BorderMode::Center, BorderMode::Inside);
        assert_ne!(BorderMode::Inside, BorderMode::Outside);
    }

    #[test]
    fn test_to_params_default_values() {
        let sq = squircle();
        let params = sq.to_params(100.0, 50.0, 0.0);

        assert_eq!(params.width, 100.0);
        assert_eq!(params.height, 50.0);
        assert!(params.corner_radius.is_none());
        assert_eq!(params.top_left_corner_radius, Some(0.0));
        assert_eq!(params.top_right_corner_radius, Some(0.0));
        assert_eq!(params.bottom_left_corner_radius, Some(0.0));
        assert_eq!(params.bottom_right_corner_radius, Some(0.0));
        assert_eq!(params.corner_smoothing, 1.0);
        assert!(params.preserve_smoothing);
    }

    #[test]
    fn test_to_params_with_border_offset() {
        let sq = squircle();
        let params = sq.to_params(100.0, 50.0, 5.0);

        // Border offset is added to corner radii
        assert_eq!(params.top_left_corner_radius, Some(5.0));
        assert_eq!(params.top_right_corner_radius, Some(5.0));
        assert_eq!(params.bottom_left_corner_radius, Some(5.0));
        assert_eq!(params.bottom_right_corner_radius, Some(5.0));
    }

    #[test]
    fn test_to_params_with_custom_corner_radii() {
        let mut sq = squircle();
        sq.style.corner_radii.top_left = Some(px(10.));
        sq.style.corner_radii.top_right = Some(px(20.));
        sq.style.corner_radii.bottom_left = Some(px(30.));
        sq.style.corner_radii.bottom_right = Some(px(40.));

        let params = sq.to_params(100.0, 100.0, 0.0);

        assert_eq!(params.top_left_corner_radius, Some(10.0));
        assert_eq!(params.top_right_corner_radius, Some(20.0));
        assert_eq!(params.bottom_left_corner_radius, Some(30.0));
        assert_eq!(params.bottom_right_corner_radius, Some(40.0));
    }

    #[test]
    fn test_to_params_with_corner_smoothing() {
        let mut sq = squircle();
        sq.style.corner_smoothing = Some(px(0.6));

        let params = sq.to_params(100.0, 100.0, 0.0);

        assert_eq!(params.corner_smoothing, 0.6);
    }

    #[test]
    fn test_to_params_preserve_smoothing_false() {
        let mut sq = squircle();
        sq.style.preserve_smoothing = Some(false);

        let params = sq.to_params(100.0, 100.0, 0.0);

        assert!(!params.preserve_smoothing);
    }

    #[test]
    fn test_get_size_and_border_offsets_center() {
        let sq = squircle();
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(4.0);

        assert_eq!(size_offset, 0.0);
        assert_eq!(border_offset, 0.0);
    }

    #[test]
    fn test_get_size_and_border_offsets_outside() {
        let mut sq = squircle();
        sq.style.border_mode = Some(BorderMode::Outside);

        let (size_offset, border_offset) = sq.get_size_and_border_offsets(4.0);

        assert_eq!(size_offset, -4.0);
        assert_eq!(border_offset, 2.0); // border_width - 2
    }

    #[test]
    fn test_get_size_and_border_offsets_inside() {
        let mut sq = squircle();
        sq.style.border_mode = Some(BorderMode::Inside);

        let (size_offset, border_offset) = sq.get_size_and_border_offsets(4.0);

        assert_eq!(size_offset, 4.0);
        assert_eq!(border_offset, -3.0); // -(border_width - 1)
    }

    #[test]
    fn test_build_lyon_path_valid() {
        let mut sq = squircle();
        sq.style.corner_radii.top_left = Some(px(10.));
        sq.style.corner_radii.top_right = Some(px(10.));
        sq.style.corner_radii.bottom_left = Some(px(10.));
        sq.style.corner_radii.bottom_right = Some(px(10.));

        let path = sq.build_lyon_path(size(px(100.), px(100.)), 0.0);

        assert!(path.is_some());
    }

    #[test]
    fn test_build_lyon_path_zero_size() {
        let sq = squircle();
        let path = sq.build_lyon_path(size(px(0.), px(0.)), 0.0);

        // Should still produce a valid (empty) path
        assert!(path.is_some());
    }

    #[test]
    fn test_build_lyon_path_large_corner_radius() {
        let mut sq = squircle();
        // Corner radius larger than half the size
        sq.style.corner_radii.top_left = Some(px(100.));
        sq.style.corner_radii.top_right = Some(px(100.));
        sq.style.corner_radii.bottom_left = Some(px(100.));
        sq.style.corner_radii.bottom_right = Some(px(100.));

        let path = sq.build_lyon_path(size(px(50.), px(50.)), 0.0);

        // Should handle gracefully
        assert!(path.is_some());
    }

    #[test]
    fn test_squircle_style_refinement_default() {
        let style = SquircleStyleRefinement::default();

        assert!(style.background.is_none());
        assert!(style.border_color.is_none());
        assert!(style.border_width.is_none());
        assert!(style.border_mode.is_none());
        assert!(style.corner_smoothing.is_none());
        assert!(style.preserve_smoothing.is_none());
        assert!(style.corner_radii.top_left.is_none());
        assert!(style.corner_radii.top_right.is_none());
        assert!(style.corner_radii.bottom_left.is_none());
        assert!(style.corner_radii.bottom_right.is_none());
    }

    #[test]
    fn test_element_id_none_by_default() {
        let sq = squircle();
        assert!(Element::id(&sq).is_none());
    }

    #[test]
    fn test_asymmetric_corner_radii() {
        let mut sq = squircle();
        sq.style.corner_radii.top_left = Some(px(5.));
        sq.style.corner_radii.top_right = Some(px(15.));
        sq.style.corner_radii.bottom_left = Some(px(25.));
        sq.style.corner_radii.bottom_right = Some(px(35.));

        let params = sq.to_params(200.0, 100.0, 0.0);

        assert_eq!(params.top_left_corner_radius, Some(5.0));
        assert_eq!(params.top_right_corner_radius, Some(15.0));
        assert_eq!(params.bottom_left_corner_radius, Some(25.0));
        assert_eq!(params.bottom_right_corner_radius, Some(35.0));
    }

    #[test]
    fn test_border_offsets_with_zero_width() {
        let sq = squircle();
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(0.0);

        assert_eq!(size_offset, 0.0);
        assert_eq!(border_offset, 0.0);
    }

    #[test]
    fn test_border_offsets_outside_with_various_widths() {
        let mut sq = squircle();
        sq.style.border_mode = Some(BorderMode::Outside);

        // Test with 1px border
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(1.0);
        assert_eq!(size_offset, -1.0);
        assert_eq!(border_offset, -1.0);

        // Test with 2px border
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(2.0);
        assert_eq!(size_offset, -2.0);
        assert_eq!(border_offset, 0.0);

        // Test with 10px border
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(10.0);
        assert_eq!(size_offset, -10.0);
        assert_eq!(border_offset, 8.0);
    }

    #[test]
    fn test_border_offsets_inside_with_various_widths() {
        let mut sq = squircle();
        sq.style.border_mode = Some(BorderMode::Inside);

        // Test with 1px border
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(1.0);
        assert_eq!(size_offset, 1.0);
        assert_eq!(border_offset, 0.0);

        // Test with 2px border
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(2.0);
        assert_eq!(size_offset, 2.0);
        assert_eq!(border_offset, -1.0);

        // Test with 10px border
        let (size_offset, border_offset) = sq.get_size_and_border_offsets(10.0);
        assert_eq!(size_offset, 10.0);
        assert_eq!(border_offset, -9.0);
    }

    #[test]
    fn test_negative_border_offset_in_params() {
        let mut sq = squircle();
        sq.style.corner_radii.top_left = Some(px(20.));

        // Negative offset should reduce corner radius
        let params = sq.to_params(100.0, 100.0, -5.0);

        assert_eq!(params.top_left_corner_radius, Some(15.0));
    }
}
