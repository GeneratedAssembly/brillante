use leptos::prelude::*;
use tachys::view::Position;
use tachys::view::RenderHtml;

pub fn render_html<V: RenderHtml + 'static>(build: impl FnOnce() -> V) -> String {
    let owner = Owner::new();
    let mut buf = owner.with(move || {
        let view = build();
        let mut buf = String::new();
        let mut pos = Position::FirstChild;
        view.to_html_with_buf(&mut buf, &mut pos, true, false, Vec::new());
        buf
    });
    if buf.contains("<!>") {
        buf = buf.replace("<!>", "");
    }
    buf
}

pub fn render_document<V: RenderHtml + 'static>(build: impl FnOnce() -> V) -> String {
    format!("<!DOCTYPE html>{}", render_html(build))
}

#[cfg(test)]
mod tests {
    use leptos::prelude::*;

    #[tokio::test]
    async fn escapes_and_strips_separators() {
        let html = crate::render::render_html(move || {
            view! { <p>"a & b "</p><p>"c"</p> }
        });
        assert_eq!(html, "<p>a &amp; b </p><p>c</p>");
        assert!(!html.contains("<!>"));
    }
}
