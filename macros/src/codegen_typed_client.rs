//! Code generation for typed MQTT clients

use quote::{quote, format_ident};

use crate::{
    codegen::CodeGenerator,
    naming::TypedClientNames,
};

/// Generates typed client extensions for MQTT message types
pub struct TypedClientGenerator<'a> {
    code_generator: &'a CodeGenerator,
    names: TypedClientNames,
    struct_name: syn::Ident,
}

impl<'a> TypedClientGenerator<'a> {
    /// Create new generator with code generator reference and struct name
    pub fn new(code_generator: &'a CodeGenerator, struct_name: &syn::Ident) -> Self {
        let names = TypedClientNames::from_struct_name(struct_name);
        Self { 
            code_generator, 
            names, 
            struct_name: struct_name.clone(),
        }
    }

    /// Generate complete typed client implementation
    pub fn generate_complete_typed_client(&self) -> proc_macro2::TokenStream {
        let client_struct = self.generate_typed_client_struct();
        let client_impl = self.generate_typed_client_impl();
        let extension_trait = self.generate_extension_trait();
        let extension_impl = self.generate_extension_impl();

        quote! {
            #client_struct
            #client_impl
            #extension_trait
            #extension_impl
        }
    }

    /// Generate typed client struct: `SensorMessageClient<F>`
    fn generate_typed_client_struct(&self) -> proc_macro2::TokenStream {
        let client_struct = &self.names.client_struct;

        quote! {
            pub struct #client_struct<F> {
                client: ::mqtt_typed_client_core::MqttClient<F>,
            }
        }
    }

    /// Generate implementation block for typed client
    fn generate_typed_client_impl(&self) -> proc_macro2::TokenStream {
        let client_struct = &self.names.client_struct;
        let payload_type = self.code_generator.get_payload_type_token();
        
        let publisher_methods = 
            if self.code_generator.should_generate_publisher() { 
                self.generate_all_publisher_methods()
            } else {  
                quote! {}
            };
        let subscribe_method = 
            if self.code_generator.should_generate_subscriber() {
                self.generate_subscribe_method()
            } else {
                quote! {}
            };
        quote! {
            impl<F> #client_struct<F>
            where 
                F: ::mqtt_typed_client_core::MessageSerializer<#payload_type>
            {
                #publisher_methods
                #subscribe_method
            }
        }
    }

    /// Generate extension trait: `SensorMessageExt<F>`
    fn generate_extension_trait(&self) -> proc_macro2::TokenStream {
        let extension_trait = &self.names.extension_trait;
        let client_struct = &self.names.client_struct;
        let method_name = format_ident!("{}", self.names.method_name);

        quote! {
            pub trait #extension_trait<F> {
                fn #method_name(&self) -> #client_struct<F>;
            }
        }
    }

    /// Generate extension trait implementation for MqttClient
    fn generate_extension_impl(&self) -> proc_macro2::TokenStream {
        let extension_trait = &self.names.extension_trait;
        let client_struct = &self.names.client_struct;
        let method_name = format_ident!("{}", self.names.method_name);

        quote! {
            impl<F> #extension_trait<F> for ::mqtt_typed_client_core::MqttClient<F> 
            where 
                F: Clone
            {
                fn #method_name(&self) -> #client_struct<F> {
                    #client_struct {
                        client: self.clone(),
                    }
                }
            }
        }
    }

    /// Generate all publisher methods for typed client
    fn generate_all_publisher_methods(&self) -> proc_macro2::TokenStream {
        let payload_type = self.code_generator.get_payload_type_token();
        let method_params = self.code_generator.get_publisher_method_params();
        let struct_name = &self.struct_name;
        
        // Reuse existing logic for parameter arguments
        let (_, param_args) = self.code_generator.get_topic_format_and_args();

        quote! {
            #[allow(clippy::ptr_arg)]
            pub async fn publish(
                &self,
                #(#method_params,)*
                data: &#payload_type,
            ) -> ::std::result::Result<(), ::mqtt_typed_client_core::MqttClientError> {
                #struct_name::publish(&self.client #(, #param_args)*, data).await
            }

            pub fn get_publisher(
                &self,
                #(#method_params,)*
            ) -> ::std::result::Result<
                ::mqtt_typed_client_core::MqttPublisher<#payload_type, F>,
                ::mqtt_typed_client_core::TopicError,
            > {
                #struct_name::get_publisher(&self.client #(, #param_args)*)
            }

            pub fn get_publisher_to(
                &self,
                custom_pattern: impl TryInto<
                    ::mqtt_typed_client_core::TopicPatternPath,
                    Error = ::mqtt_typed_client_core::TopicPatternError,
                >,
                #(#method_params,)*
            ) -> ::std::result::Result<
                ::mqtt_typed_client_core::MqttPublisher<#payload_type, F>,
                ::mqtt_typed_client_core::TopicError,
            > {
                #struct_name::get_publisher_to(&self.client, custom_pattern #(, #param_args)*)
            }
        }
    }

    /// Generate subscribe method for typed client
    fn generate_subscribe_method(&self) -> proc_macro2::TokenStream {
        let payload_type = self.code_generator.get_payload_type_token();
        let struct_name = &self.struct_name;

        quote! {
            pub async fn subscribe(
                &self,
            ) -> ::std::result::Result<
                ::mqtt_typed_client_core::MqttTopicSubscriber<#struct_name, #payload_type, F>,
                ::mqtt_typed_client_core::MqttClientError,
            >
            where
                #struct_name: ::mqtt_typed_client_core::FromMqttMessage<#payload_type, F::DeserializeError>,
            {
                #struct_name::subscribe(&self.client).await
            }

            pub fn subscription(&self) -> ::mqtt_typed_client_core::SubscriptionBuilder<#struct_name, F> 
            {
                #struct_name::subscription(&self.client)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::StructAnalysisContext;
    use quote::format_ident;

    fn create_test_code_generator() -> crate::codegen::CodeGenerator {
        let context = StructAnalysisContext::from_components(
            Some(syn::parse_quote! { f64 }),
            false,
            vec![],
        );
        let macro_args = crate::test_helpers::create_test_macro_args();
        crate::codegen::CodeGenerator::new(context, macro_args)
    }

    fn create_test_struct_name() -> syn::Ident {
        quote::format_ident!("TestMessage")
    }

    #[test]
    fn test_typed_client_names_generation() {
        let code_generator = create_test_code_generator();
        let struct_name = format_ident!("SensorMessage");
        let generator = TypedClientGenerator::new(&code_generator, &struct_name);

        assert_eq!(generator.names.method_name, "sensor_message");
        assert_eq!(generator.names.client_struct.to_string(), "SensorMessageClient");
        assert_eq!(generator.names.extension_trait.to_string(), "SensorMessageExt");
    }

    #[test]
    fn test_generate_typed_client_struct() {
        let code_generator = create_test_code_generator();
        let struct_name = create_test_struct_name();
        let generator = TypedClientGenerator::new(&code_generator, &struct_name);

        let result = generator.generate_typed_client_struct();
        let expected = quote! {
            pub struct TestMessageClient<F> {
                client: ::mqtt_typed_client_core::MqttClient<F>,
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }
}
