//! Guest constants discovered from the same LVGL exports as host bindings.
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Item;

pub fn generate(source: &str) -> syn::Result<TokenStream> {
    let file = syn::parse_file(source)?;
    let mut events = Vec::new();
    let mut states = Vec::new();
    let mut types = Vec::new();
    for item in file.items {
        match item {
            Item::Const(item) => {
                let name = item.ident.to_string();
                let value = item.expr;
                if let Some(name) = name.strip_prefix("lv_event_code_t_LV_EVENT_") {
                    let name = match name {
                        "REFR_EXT_DRAW_SIZE" => "REFRESH_EXT_DRAW_SIZE",
                        "REFR_REQUEST" => "REFRESH_REQUEST",
                        name => name,
                    };
                    let variant = format_ident!(
                        "{}",
                        crate::naming::snake_to_upper_camel_case(&name.to_lowercase())
                    );
                    events.push(quote! { #variant = #value });
                } else if let Some(name) = name.strip_prefix("lv_state_t_LV_STATE_") {
                    let name = format_ident!("STATE_{name}");
                    states.push(quote! { pub const #name: State = #value; });
                }
            }
            Item::Type(item) => {
                let name = match item.ident.to_string().as_str() {
                    "lv_state_t" => Some("State"),
                    "lv_blur_quality_t" => Some("BlurQuality"),
                    "lv_text_leading_trim_t" => Some("TextLeadingTrim"),
                    _ => None,
                };
                if let Some(name) = name {
                    let name = format_ident!("{name}");
                    let ty = item.ty;
                    types.push(quote! { pub type #name = #ty; });
                }
            }
            _ => {}
        }
    }
    Ok(quote! {
        #[repr(C)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum EventCode { #(#events,)* }
        #(#types)*
        #(#states)*
    })
}
