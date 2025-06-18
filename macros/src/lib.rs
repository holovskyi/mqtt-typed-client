use mqtt_typed_client::routing::subscription_manager::CacheStrategy;
use mqtt_typed_client::topic::topic_pattern_path::TopicPatternPath;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, Fields, LitStr, parse_macro_input};

#[proc_macro_attribute]
pub fn mqtt_topic_subscriber(
	args: TokenStream,
	input: TokenStream,
) -> TokenStream {
	let topic_pattern_str = parse_macro_input!(args as LitStr);
	let input_struct = parse_macro_input!(input as syn::DeriveInput);

	let result = (|| -> Result<proc_macro2::TokenStream, syn::Error> {
		let topic_pattern = parse_topic_pattern(&topic_pattern_str)?;
		let struct_info = validate_struct_fields_and_extract_info(
			&input_struct,
			&topic_pattern,
		)?;

		let from_mqtt_impl = generate_from_mqtt_impl(
			&input_struct.ident,
			&struct_info,
			&topic_pattern,
		);
		let methods =
			generate_impl(&input_struct.ident, &topic_pattern, &struct_info.payload_type);
		Ok(quote! {
			#input_struct
			#from_mqtt_impl
			#methods
		})
	})();
	match result {
		| Ok(tokens) => tokens.into(),
		| Err(err) => err.to_compile_error().into(),
	}
}

#[derive(Debug)]
struct StructFieldInfo {
	payload_type: Option<syn::Type>,
	has_topic_field: bool,
	topic_param_fields: Vec<(String, syn::Type)>, // (param_name, field_type)
}

fn validate_struct_fields_and_extract_info(
	input: &syn::DeriveInput,
	topic_pattern: &TopicPatternPath,
) -> Result<StructFieldInfo, syn::Error> {
	// Витягуємо іменовані параметри з топік паттерна
	let topic_params: Vec<String> = topic_pattern
		.iter()
		.filter_map(|item| item.param_name())
		.map(|name| name.to_string())
		.collect();

	// Перевіряємо що це struct з named fields
	let fields = match &input.data {
		| Data::Struct(data_struct) => match &data_struct.fields {
			| Fields::Named(fields) => &fields.named,
			| _ => {
				return Err(syn::Error::new_spanned(
					input,
					"mqtt_topic_subscriber can only be applied to structs \
					 with named fields",
				));
			}
		},
		| _ => {
			return Err(syn::Error::new_spanned(
				input,
				"mqtt_topic_subscriber can only be applied to structs",
			));
		}
	};

	let mut payload_type = None;
	let mut has_topic_field = false;
	let mut topic_param_fields = Vec::new();
	let mut unknown_fields = Vec::new();

	// Аналізуємо кожне поле
	for field in fields {
		if let Some(field_name) = &field.ident {
			let field_name_str = field_name.to_string();

			match field_name_str.as_str() {
				| "payload" => {
					payload_type = Some(field.ty.clone());
				}
				| "topic" => {
					// Перевіряємо що тип Arc<TopicMatch>
					if !is_topic_match_type(&field.ty) {
						return Err(syn::Error::new_spanned(
							&field.ty,
							"Field 'topic' must be of type Arc<TopicMatch>",
						));
					}
					has_topic_field = true;
				}
				| _ => {
					// Перевіряємо чи це параметр з топіка
					if topic_params.contains(&field_name_str) {
						topic_param_fields
							.push((field_name_str, field.ty.clone()));
					} else {
						unknown_fields.push(field_name_str);
					}
				}
			}
		}
	}

	// Перевіряємо що немає невідомих полів
	if !unknown_fields.is_empty() {
		return Err(syn::Error::new_spanned(
			input,
			format!(
				"Unknown fields: {}. Only 'payload', 'topic', and topic \
				 parameters [{}] are allowed",
				unknown_fields.join(", "),
				topic_params.join(", ")
			),
		));
	}

	Ok(StructFieldInfo {
		payload_type,
		has_topic_field,
		topic_param_fields,
	})
}

fn is_topic_match_type(ty: &syn::Type) -> bool {
	// Find Arc<TopicMatch>
	if let syn::Type::Path(type_path) = ty {
		if let Some(segment) = type_path.path.segments.last() {
			if segment.ident == "Arc" {
				if let syn::PathArguments::AngleBracketed(args) =
					&segment.arguments
				{
					if let Some(syn::GenericArgument::Type(syn::Type::Path(
						inner_path,
					))) = args.args.first()
					{
						if let Some(inner_segment) =
							inner_path.path.segments.last()
						{
							return inner_segment.ident == "TopicMatch";
						}
					}
				}
			}
		}
	}
	false
}

fn parse_topic_pattern(
	topic_pattern_str: &LitStr,
) -> Result<TopicPatternPath, syn::Error> {
	TopicPatternPath::new_from_string(
		topic_pattern_str.value(),
		CacheStrategy::NoCache,
	)
	.map_err(|err| {
		syn::Error::new_spanned(
			topic_pattern_str,
			format!("Topic pattern error: {}", err),
		)
	})
}

fn generate_from_mqtt_impl(
	struct_name: &syn::Ident,
	struct_field_info: &StructFieldInfo,
	topic_pattern: &TopicPatternPath,
) -> proc_macro2::TokenStream {
	// Витягуємо параметри з топік паттерна
	let topic_params: Vec<_> = topic_pattern
		.iter()
		.filter(|item| item.is_wildcard())
		.map(|item| item.param_name().map(|name| name.to_string()))
		.collect();

	// Генеруємо код для витягування параметрів з топіка
	let param_extractions: Vec<proc_macro2::TokenStream> = struct_field_info
		.topic_param_fields
		.iter()
		.map(|(param_name, _field_type)| {
			// Знаходимо індекс параметра в топік паттерні
			let param_index = topic_params
				.iter()
				.position(|p| p.as_ref() == Some(param_name))
				.unwrap_or_else(|| panic!("Parameter {param_name} must exist in topic pattern"));
			
			let param_ident = format_ident!("{}", param_name);
			
			quote! {
				let #param_ident = ::mqtt_typed_client::extract_topic_parameter(
					&topic, 
					#param_index, 
					#param_name
				)?;
			}
		})
		.collect();

	// Генеруємо список полів для конструктора
	let field_assignments: Vec<proc_macro2::TokenStream> = struct_field_info
		.topic_param_fields
		.iter()
		.map(|(param_name, _)| {
			let param_ident = format_ident!("{}", param_name);
			quote! { #param_ident }
		})
		.collect();

	// Додаємо payload якщо є
	let payload_assignment = if struct_field_info.payload_type.is_some() {
		quote! { payload, }
	} else {
		quote! {}
	};

	// Додаємо topic якщо є
	let topic_assignment = if struct_field_info.has_topic_field {
		quote! { topic, }
	} else {
		quote! {}
	};

	// Визначаємо тип payload для trait bounds
	let payload_type = struct_field_info
		.payload_type
		.as_ref()
		.map(|ty| quote! { #ty })
		.unwrap_or_else(|| quote! { Vec<u8> });

	quote! {
		impl<DE> ::mqtt_typed_client::FromMqttMessage<#payload_type, DE> for #struct_name {
			fn from_mqtt_message(
				topic: ::std::sync::Arc<::mqtt_typed_client::topic::topic_match::TopicMatch>,
				payload: #payload_type,
			) -> ::std::result::Result<Self, ::mqtt_typed_client::MessageConversionError<DE>> {
				#(#param_extractions)*

				Ok(Self {
					#(#field_assignments,)*
					#payload_assignment
					#topic_assignment
				})
			}
		}
	}
}

fn generate_impl(
	struct_name: &syn::Ident,
	topic_pattern: &TopicPatternPath,
	payload_type: &Option<syn::Type>,
) -> proc_macro2::TokenStream {
	// Визначаємо тип payload - якщо немає, то Vec<u8> як fallback
	let payload_type_token = payload_type
		.as_ref()
		.map(|ty| quote! { #ty })
		.unwrap_or_else(|| quote! { Vec<u8> });
	let topic_pattern_literal = topic_pattern.topic_pattern().to_string();
	let mqtt_pattern_literal = topic_pattern.mqtt_pattern().to_string();

	quote! {
		impl #struct_name {
			/// The topic pattern used for this subscriber
			pub const TOPIC_PATTERN: &'static str = #topic_pattern_literal;

			/// The MQTT pattern for subscription
			pub const MQTT_PATTERN: &'static str = #mqtt_pattern_literal;
			
			/// Subscribe to this topic pattern using the provided MQTT client
			pub async fn subscribe<F>(
				client: &::mqtt_typed_client::MqttClient<F>,
			) -> ::std::result::Result <
				::mqtt_typed_client::MqttStructuredSubscriber<Self, #payload_type_token, F>,
				::mqtt_typed_client::MqttClientError,
			>
			where 
				F: ::std::default::Default 
					+ ::std::clone::Clone 
					+ ::std::marker::Send 
					+ ::std::marker::Sync 
					+ ::mqtt_typed_client::MessageSerializer<#payload_type_token>,
			{
				let subscriber = client.subscribe::<#payload_type_token>(Self::MQTT_PATTERN).await?;
				Ok(::mqtt_typed_client::MqttStructuredSubscriber::new(subscriber))
			}
		}
	}
}