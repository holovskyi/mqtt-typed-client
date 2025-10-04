//! Internal documentation transformation macros
//!
//! ⚠️ This crate is for internal use only and should not be used directly.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod transform;

/// Includes and transforms a Markdown file for rustdoc
///
/// # Internal Use Only
///
/// This macro is used internally by mqtt-typed-client to transform
/// documentation links from GitHub format to rustdoc format.
///
/// # Syntax
///
/// ```ignore
/// include_md_transformed!("path/to/file.md")
/// include_md_transformed!("path/to/file.md", transform = "examples")
/// include_md_transformed!("path/to/file.md", transform = "readme")
/// ```
///
/// # Transform types
///
/// - `"examples"` - Transforms links like `[file.rs](path#example)` to rustdoc format
/// - `"readme"` - Transforms links like `[examples/](examples/)` to `[crate::examples]`
/// - No transform parameter - Returns file content as-is
#[proc_macro]
pub fn include_md_transformed(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as transform::TransformInput);

	match transform::process_markdown(&input) {
		| Ok(content) => {
			let expanded = quote! {
				#content
			};
			TokenStream::from(expanded)
		}
		| Err(err) => {
			let error = err.to_string();
			let expanded = quote! {
				compile_error!(#error)
			};
			TokenStream::from(expanded)
		}
	}
}
