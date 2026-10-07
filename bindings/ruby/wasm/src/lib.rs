//! C ABI used by the JRuby WebAssembly backend.

#![allow(clippy::missing_safety_doc)]

use std::alloc::{alloc, dealloc, handle_alloc_error, Layout};
use std::borrow::Cow;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use css_inline::{CSSInliner, InlineOptions, Url};
use serde::Deserialize;

#[repr(C)]
pub struct CssInlineResult {
    pub value: *mut c_char,
    pub error: *mut c_char,
}

impl CssInlineResult {
    fn ok(value: String) -> Self {
        Self {
            value: into_c_string(value),
            error: ptr::null_mut(),
        }
    }

    fn err(error: impl ToString) -> Self {
        Self {
            value: ptr::null_mut(),
            error: into_c_string(error.to_string()),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Options {
    inline_style_tags: Option<bool>,
    keep_style_tags: Option<bool>,
    keep_link_tags: Option<bool>,
    keep_at_rules: Option<bool>,
    minify_css: Option<bool>,
    base_url: Option<String>,
    load_remote_stylesheets: Option<bool>,
    extra_css: Option<String>,
    preallocate_node_capacity: Option<usize>,
    remove_inlined_selectors: Option<bool>,
    apply_width_attributes: Option<bool>,
    apply_height_attributes: Option<bool>,
}

impl Options {
    fn into_inline_options(self) -> Result<InlineOptions<'static>, String> {
        let base_url = self
            .base_url
            .map(|value| Url::parse(&value).map_err(|error| format!("{error}: {value}")))
            .transpose()?;

        Ok(InlineOptions {
            inline_style_tags: self.inline_style_tags.unwrap_or(true),
            keep_style_tags: self.keep_style_tags.unwrap_or(false),
            keep_link_tags: self.keep_link_tags.unwrap_or(false),
            keep_at_rules: self.keep_at_rules.unwrap_or(false),
            minify_css: self.minify_css.unwrap_or(false),
            base_url,
            load_remote_stylesheets: self.load_remote_stylesheets.unwrap_or(true),
            extra_css: self.extra_css.map(Cow::Owned),
            preallocate_node_capacity: self.preallocate_node_capacity.unwrap_or(32),
            remove_inlined_selectors: self.remove_inlined_selectors.unwrap_or(false),
            apply_width_attributes: self.apply_width_attributes.unwrap_or(false),
            apply_height_attributes: self.apply_height_attributes.unwrap_or(false),
            ..InlineOptions::default()
        })
    }
}

fn into_c_string(value: String) -> *mut c_char {
    CString::new(value).unwrap_or_default().into_raw()
}

unsafe fn borrow_str<'a>(pointer: *const c_char, label: &str) -> Result<&'a str, String> {
    if pointer.is_null() {
        return Err(format!("{label} is null"));
    }

    CStr::from_ptr(pointer)
        .to_str()
        .map_err(|error| format!("Invalid UTF-8 in {label}: {error}"))
}

unsafe fn borrow_options(pointer: *const c_char) -> Result<InlineOptions<'static>, String> {
    if pointer.is_null() {
        return Ok(InlineOptions::default());
    }

    let json = borrow_str(pointer, "options")?;
    let options = if json.trim().is_empty() {
        Options::default()
    } else {
        serde_json::from_str(json).map_err(|error| format!("Invalid options: {error}"))?
    };

    options.into_inline_options()
}

fn inline_source(html: &str, options: InlineOptions<'static>) -> Result<String, String> {
    CSSInliner::new(options)
        .inline(html)
        .map_err(|error| error.to_string())
}

fn inline_fragment_source(
    html: &str,
    css: &str,
    options: InlineOptions<'static>,
) -> Result<String, String> {
    CSSInliner::new(options)
        .inline_fragment(html, css)
        .map_err(|error| error.to_string())
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_inline(
    html: *const c_char,
    options_json: *const c_char,
) -> CssInlineResult {
    let result = borrow_str(html, "html").and_then(|html| {
        borrow_options(options_json).and_then(|options| inline_source(html, options))
    });

    match result {
        Ok(value) => CssInlineResult::ok(value),
        Err(error) => CssInlineResult::err(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_inline_fragment(
    html: *const c_char,
    css: *const c_char,
    options_json: *const c_char,
) -> CssInlineResult {
    let result = borrow_str(html, "html").and_then(|html| {
        borrow_str(css, "css").and_then(|css| {
            borrow_options(options_json)
                .and_then(|options| inline_fragment_source(html, css, options))
        })
    });

    match result {
        Ok(value) => CssInlineResult::ok(value),
        Err(error) => CssInlineResult::err(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_validate_options(
    options_json: *const c_char,
) -> CssInlineResult {
    match borrow_options(options_json) {
        Ok(_) => CssInlineResult::ok(String::new()),
        Err(error) => CssInlineResult::err(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_result_free(result: CssInlineResult) {
    css_inline_string_free(result.value);
    css_inline_string_free(result.error);
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_allocate(size: usize) -> *mut u8 {
    if size == 0 {
        return ptr::null_mut();
    }

    let layout = Layout::from_size_align(size, 8).expect("valid allocation layout");
    let allocation = alloc(layout);

    if allocation.is_null() {
        handle_alloc_error(layout);
    }

    allocation
}

#[no_mangle]
pub unsafe extern "C" fn css_inline_deallocate(allocation: *mut u8, size: usize) {
    if allocation.is_null() || size == 0 {
        return;
    }

    let layout = Layout::from_size_align(size, 8).expect("valid allocation layout");
    dealloc(allocation, layout);
}

#[cfg(test)]
mod tests {
    use super::{inline_fragment_source, Options};

    #[test]
    fn inlines_a_fragment() {
        let options = Options::default().into_inline_options().unwrap();
        let output = inline_fragment_source("<p>Hello</p>", "p { color: red; }", options).unwrap();

        assert_eq!(output, "<p style=\"color: red;\">Hello</p>");
    }

    #[test]
    fn applies_options() {
        let options: Options = serde_json::from_str(r#"{"keep_style_tags":true}"#).unwrap();
        let output = super::inline_source(
            "<style>p { color: red; }</style><p>Hello</p>",
            options.into_inline_options().unwrap(),
        )
        .unwrap();

        assert_eq!(
            output,
            "<html><head><style>p { color: red; }</style></head><body><p style=\"color: red;\">Hello</p></body></html>"
        );
    }

    #[test]
    fn rejects_a_relative_base_url() {
        let options: Options = serde_json::from_str(r#"{"base_url":"foo"}"#).unwrap();

        assert_eq!(
            options.into_inline_options().unwrap_err(),
            "relative URL without a base: foo"
        );
    }
}
