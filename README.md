# GPUI Squircle

> A squircle component for gpui.

![rounded rect vs squircle](rounded_rect_vs_squircle.png)

Install via [crates.io](https://crates.io/crates/gpui_squircle)

## Usage

```rs
use gpui::{ParentElement, Styled, div, px};
use gpui_squircle::{squircle, SquircleStylable};

fn squircle_div() -> impl gpui::IntoElement {
    div()
        .size(px(200.))

        .child(
            // To use a squircle simply parent it to an element.
            // It automatically fills the parent's entire size
            // whilst also ignoring padding.
            squircle()
                .rounded(px(25.))
                .bg(gpui::red())
                .border(px(15.))
                .border_color(gpui::blue())
                .border_outside()
        )
}
```

## Examples

Examples can be found [here](/examples)
