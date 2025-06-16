use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn mqtt_topic_subscriber(_args: TokenStream, input: TokenStream) -> TokenStream {
    input
}