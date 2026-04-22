use comrak::Options;

pub const TRUST_MODEL: &str = "trusted-local-markdown-preview";

pub fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.autolink = true;
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.tasklist = true;

    // mdv intentionally preserves raw HTML because this renderer is only for
    // trusted local Markdown opened by the user. Anything returned across this
    // boundary must only be injected by the trusted preview path on the frontend.
    options.render.r#unsafe = true;
    options
}
