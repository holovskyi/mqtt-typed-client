//! Protocol-neutral connection configuration.
//!
//! [`ConnectionOptions`] replaces the previously leaked `rumqttc::MqttOptions`
//! in the public API. It deliberately mirrors only the settings most
//! applications need; every other backend knob is reachable through the
//! semver-exempt escape hatch behind the `unstable-backend-api` feature.

#[cfg(any(
	feature = "tls-rustls",
	feature = "tls-rustls-no-provider",
	feature = "unstable-backend-api"
))]
use std::sync::Arc;
use std::time::Duration;

use mqtt_topic_engine::QoS;

use super::error::{MqttClientError, UrlParseError};

/// Username/password pair sent in the MQTT CONNECT packet.
#[derive(Clone)]
pub struct Credentials {
	/// MQTT username
	pub username: String,
	/// MQTT password
	pub password: String,
}

impl std::fmt::Debug for Credentials {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Credentials")
			.field("username", &self.username)
			.field("password", &"***")
			.finish()
	}
}

/// What happens to the MQTT session across connections.
///
/// Modeled on MQTT 5 semantics (`clean_start` + session expiry) with a
/// documented MQTT 3.1.1 mapping, per the dual-protocol API design.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionPolicy {
	/// Start fresh on every connect; the broker discards state on disconnect.
	///
	/// v4: `clean_session = true`. v5: `clean_start = true`, session expiry 0.
	#[default]
	CleanPerConnection,
	/// Resume the previous session; the broker keeps state indefinitely.
	///
	/// v4: `clean_session = false`. v5: `clean_start = false`, session never
	/// expires. Requires a non-empty `client_id`.
	Resume,
	/// Resume the previous session; the broker keeps state for the given time
	/// after disconnect.
	///
	/// MQTT 5 only (session expiry interval, rounded up to whole seconds).
	/// On MQTT 3.1.1 this is not representable and `connect` returns a
	/// configuration error rather than silently degrading.
	ResumeFor(Duration),
}

/// MQTT protocol version to speak on the wire.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProtocolVersion {
	/// MQTT 3.1.1 (protocol level 4)
	#[default]
	V4,
	/// MQTT 5.0 — accepted in configuration now; wire support arrives in 0.4.
	V5,
}

/// TLS configuration for encrypted transports.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub enum TlsConfig {
	/// Backend defaults: rustls with the platform's native root certificates.
	/// Requires the `tls-rustls` (or `tls-rustls-no-provider`) feature.
	#[default]
	Default,
	/// A caller-supplied rustls `ClientConfig` (custom roots, client auth, …).
	Rustls(RustlsClientConfig),
}

/// Opaque holder for a rustls client configuration.
///
/// The type is always present so that [`TlsConfig`]'s shape never depends on
/// cargo features; constructing a non-trivial value requires the `tls-rustls`
/// (or `tls-rustls-no-provider`) feature, which gates the `From` impls and
/// re-exports the `rustls` crate at the crate root.
///
/// Note: the wrapped `rustls` major version tracks the backend's TLS stack —
/// a documented semver-coupled exception (see the crate docs).
#[derive(Clone)]
pub struct RustlsClientConfig {
	#[cfg(any(feature = "tls-rustls", feature = "tls-rustls-no-provider"))]
	pub(crate) config: Arc<rumqttc::tokio_rustls::rustls::ClientConfig>,
	#[cfg(not(any(
		feature = "tls-rustls",
		feature = "tls-rustls-no-provider"
	)))]
	pub(crate) _unconstructible: std::convert::Infallible,
}

impl std::fmt::Debug for RustlsClientConfig {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("RustlsClientConfig(..)")
	}
}

#[cfg(any(feature = "tls-rustls", feature = "tls-rustls-no-provider"))]
impl From<Arc<rumqttc::tokio_rustls::rustls::ClientConfig>>
	for RustlsClientConfig
{
	fn from(config: Arc<rumqttc::tokio_rustls::rustls::ClientConfig>) -> Self {
		Self { config }
	}
}

#[cfg(any(feature = "tls-rustls", feature = "tls-rustls-no-provider"))]
impl From<rumqttc::tokio_rustls::rustls::ClientConfig> for RustlsClientConfig {
	fn from(config: rumqttc::tokio_rustls::rustls::ClientConfig) -> Self {
		Self {
			config: Arc::new(config),
		}
	}
}

#[cfg(any(feature = "tls-rustls", feature = "tls-rustls-no-provider"))]
impl From<rumqttc::tokio_rustls::rustls::ClientConfig> for TlsConfig {
	fn from(config: rumqttc::tokio_rustls::rustls::ClientConfig) -> Self {
		TlsConfig::Rustls(config.into())
	}
}

/// Network transport for the MQTT connection.
///
/// URL schemes map to variants (`mqtt`/`tcp` → `Tcp`, `mqtts`/`ssl` → `Tls`,
/// `ws` → `Ws`, `wss` → `Wss`). Scheme parsing always succeeds; if the needed
/// transport support is not compiled in, `connect` returns a configuration
/// error naming the missing feature.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub enum Transport {
	/// Plain TCP
	#[default]
	Tcp,
	/// TLS over TCP (requires a `tls-rustls*` feature)
	Tls(TlsConfig),
	/// WebSocket (requires the `websocket` feature)
	Ws,
	/// WebSocket over TLS (requires `websocket` + a `tls-rustls*` feature)
	Wss(TlsConfig),
}

/// Serialized Last Will message, produced by
/// [`MqttClientConfig::with_last_will`](super::config::MqttClientConfig::with_last_will).
#[derive(Debug, Clone)]
pub(crate) struct LastWillMessage {
	pub topic: String,
	pub payload: Vec<u8>,
	pub qos: QoS,
	pub retain: bool,
}

/// Protocol-neutral MQTT connection options.
///
/// Covers the settings most applications need. Backend-specific knobs that
/// are intentionally not mirrored here (inflight window, request channel
/// sizes, pending throttle, packet-size caps, proxies, …) are reachable via
/// [`ConnectionOptions::backend_tweak`] behind the `unstable-backend-api`
/// feature.
#[derive(Clone)]
pub struct ConnectionOptions {
	/// MQTT client identifier. May be empty only with
	/// [`SessionPolicy::CleanPerConnection`].
	pub client_id: String,
	/// Broker host name or IP address
	pub host: String,
	/// Broker port
	pub port: u16,
	/// Keep-alive interval. `Duration::ZERO` disables keep-alive; non-zero
	/// values must be at least one second.
	pub keep_alive: Duration,
	/// Optional username/password
	pub credentials: Option<Credentials>,
	/// Session persistence policy
	pub session: SessionPolicy,
	/// Protocol version (MQTT 5 wire support arrives in 0.4)
	pub protocol: ProtocolVersion,
	/// Network transport
	pub transport: Transport,
	pub(crate) last_will: Option<LastWillMessage>,
	#[cfg(feature = "unstable-backend-api")]
	backend_tweaks: Vec<Arc<BackendTweakFn>>,
}

#[cfg(feature = "unstable-backend-api")]
type BackendTweakFn = dyn Fn(&mut backend::BackendOptions<'_>) + Send + Sync;

impl std::fmt::Debug for ConnectionOptions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut s = f.debug_struct("ConnectionOptions");
		s.field("client_id", &self.client_id)
			.field("host", &self.host)
			.field("port", &self.port)
			.field("keep_alive", &self.keep_alive)
			.field("credentials", &self.credentials)
			.field("session", &self.session)
			.field("protocol", &self.protocol)
			.field("transport", &self.transport)
			.field("last_will", &self.last_will.as_ref().map(|w| &w.topic));
		#[cfg(feature = "unstable-backend-api")]
		s.field("backend_tweaks", &self.backend_tweaks.len());
		s.finish()
	}
}

impl ConnectionOptions {
	/// Create options with defaults: TCP transport, 60 s keep-alive,
	/// [`SessionPolicy::CleanPerConnection`], MQTT 3.1.1.
	pub fn new(
		client_id: impl Into<String>,
		host: impl Into<String>,
		port: u16,
	) -> Self {
		Self {
			client_id: client_id.into(),
			host: host.into(),
			port,
			keep_alive: Duration::from_secs(60),
			credentials: None,
			session: SessionPolicy::default(),
			protocol: ProtocolVersion::default(),
			transport: Transport::default(),
			last_will: None,
			#[cfg(feature = "unstable-backend-api")]
			backend_tweaks: Vec::new(),
		}
	}

	/// Parse connection options from an MQTT URL.
	///
	/// Grammar (compatible with rumqttc's URL grammar where mirrored):
	/// - schemes: `tcp`/`mqtt` (port 1883), `ssl`/`mqtts` (8883), `ws`/`wss`
	///   (8000 — rumqttc-compat quirk). All schemes parse regardless of
	///   compiled features; transport availability is checked at connect.
	/// - userinfo: `mqtt://user:pass@host` sets credentials.
	/// - query: `client_id` (required), `keep_alive_secs`, `clean_session`
	///   (`true` → [`SessionPolicy::CleanPerConnection`], `false` →
	///   [`SessionPolicy::Resume`]), `protocol` (`4` or `5`).
	///
	/// Backend-tuning parameters accepted by rumqttc URLs (`inflight_num`,
	/// `request_channel_capacity_num`, `max_request_batch_num`,
	/// `pending_throttle_usecs`, `max_incoming_packet_size_bytes`,
	/// `max_outgoing_packet_size_bytes`) are intentionally NOT supported —
	/// they error with a pointer to the `unstable-backend-api` escape hatch.
	/// Unknown parameters are an error.
	pub fn from_url(url: &str) -> Result<Self, UrlParseError> {
		const MOVED_TO_ESCAPE_HATCH: &[&str] = &[
			"inflight_num",
			"request_channel_capacity_num",
			"max_request_batch_num",
			"pending_throttle_usecs",
			"max_incoming_packet_size_bytes",
			"max_outgoing_packet_size_bytes",
			"conn_timeout_secs",
		];

		let url = url::Url::parse(url)
			.map_err(|e| UrlParseError::Invalid(e.to_string()))?;

		let (transport, default_port) = match url.scheme() {
			| "mqtt" | "tcp" => (Transport::Tcp, 1883),
			| "mqtts" | "ssl" => (Transport::Tls(TlsConfig::Default), 8883),
			| "ws" => (Transport::Ws, 8000),
			// rumqttc-compat: wss also defaults to 8000, not 443
			| "wss" => (Transport::Wss(TlsConfig::Default), 8000),
			| other => return Err(UrlParseError::Scheme(other.to_string())),
		};

		let host = url
			.host_str()
			.filter(|h| !h.is_empty())
			.ok_or(UrlParseError::MissingHost)?
			.to_owned();
		let port = url.port().unwrap_or(default_port);

		let credentials = match url.username() {
			| "" => None,
			| user => Some(Credentials {
				username: user.to_owned(),
				password: url.password().unwrap_or("").to_owned(),
			}),
		};

		let mut options = Self::new(String::new(), host, port);
		options.transport = transport;
		options.credentials = credentials;

		let mut client_id_seen = false;
		for (key, value) in url.query_pairs() {
			match key.as_ref() {
				| "client_id" => {
					options.client_id = value.into_owned();
					client_id_seen = true;
				}
				| "keep_alive_secs" => {
					let secs: u64 = value.parse().map_err(|_| {
						UrlParseError::InvalidParam {
							name: "keep_alive_secs".into(),
							reason: format!("`{value}` is not a number"),
						}
					})?;
					options.keep_alive = Duration::from_secs(secs);
				}
				| "clean_session" => {
					let clean: bool = value.parse().map_err(|_| {
						UrlParseError::InvalidParam {
							name: "clean_session".into(),
							reason: format!("`{value}` is not a boolean"),
						}
					})?;
					options.session = if clean {
						SessionPolicy::CleanPerConnection
					} else {
						SessionPolicy::Resume
					};
				}
				| "protocol" => {
					options.protocol = match value.as_ref() {
						| "4" => ProtocolVersion::V4,
						| "5" => ProtocolVersion::V5,
						| other => {
							return Err(UrlParseError::UnsupportedProtocol(
								other.to_string(),
							));
						}
					};
				}
				| moved if MOVED_TO_ESCAPE_HATCH.contains(&moved) => {
					return Err(UrlParseError::UnsupportedParam(
						moved.to_string(),
					));
				}
				| unknown => {
					return Err(UrlParseError::UnknownParam(
						unknown.to_string(),
					));
				}
			}
		}

		if !client_id_seen {
			return Err(UrlParseError::MissingClientId);
		}

		Ok(options)
	}

	/// Register a semver-exempt tweak applied to the raw backend options at
	/// connect time, after the facade conversion, in insertion order.
	///
	/// This is the escape hatch for backend knobs the facade does not mirror.
	/// Its signature may change with ANY backend change — it is exempt from
	/// this crate's semver guarantees. The backend crate itself is
	/// re-exported as `mqtt_typed_client_core::backend::rumqttc` so you don't
	/// need a version-matched dependency of your own.
	#[cfg(feature = "unstable-backend-api")]
	pub fn backend_tweak(
		&mut self,
		f: impl Fn(&mut backend::BackendOptions<'_>) + Send + Sync + 'static,
	) -> &mut Self {
		self.backend_tweaks.push(Arc::new(f));
		self
	}

	/// Validate and convert to rumqttc v4 options. The single choke point the
	/// 0.4 backend enum will branch on.
	pub(crate) fn to_backend_v4(
		&self,
	) -> Result<rumqttc::MqttOptions, MqttClientError> {
		if self.protocol == ProtocolVersion::V5 {
			return Err(MqttClientError::ConfigurationValue(
				"MQTT 5 wire support arrives in 0.4; use ProtocolVersion::V4"
					.into(),
			));
		}
		// rumqttc's set_keep_alive asserts (zero | >= 1s); validate instead
		// of inheriting the panic.
		if !self.keep_alive.is_zero()
			&& self.keep_alive < Duration::from_secs(1)
		{
			return Err(MqttClientError::ConfigurationValue(format!(
				"keep_alive must be zero (disabled) or at least 1 second, got \
				 {:?}",
				self.keep_alive
			)));
		}
		let clean_session = match self.session {
			| SessionPolicy::CleanPerConnection => true,
			| SessionPolicy::Resume => false,
			| SessionPolicy::ResumeFor(_) => {
				return Err(MqttClientError::ConfigurationValue(
					"SessionPolicy::ResumeFor is MQTT 5 only (session expiry \
					 interval); MQTT 3.1.1 supports CleanPerConnection or \
					 Resume"
						.into(),
				));
			}
		};
		// rumqttc's set_clean_session asserts a non-empty client_id.
		if !clean_session && self.client_id.is_empty() {
			return Err(MqttClientError::ConfigurationValue(
				"a persistent session (SessionPolicy::Resume) requires a \
				 non-empty client_id"
					.into(),
			));
		}

		let mut opts = rumqttc::MqttOptions::new(
			self.client_id.clone(),
			self.host.clone(),
			self.port,
		);
		opts.set_keep_alive(self.keep_alive);
		opts.set_clean_session(clean_session);
		opts.set_transport(self.backend_transport()?);
		if let Some(creds) = &self.credentials {
			opts.set_credentials(&creds.username, &creds.password);
		}
		if let Some(will) = &self.last_will {
			opts.set_last_will(rumqttc::LastWill::new(
				will.topic.clone(),
				will.payload.clone(),
				will.qos.to_rumqttc(),
				will.retain,
			));
		}

		#[cfg(feature = "unstable-backend-api")]
		for tweak in &self.backend_tweaks {
			let mut backend_options = backend::BackendOptions::V4(&mut opts);
			tweak(&mut backend_options);
		}

		Ok(opts)
	}

	fn backend_transport(&self) -> Result<rumqttc::Transport, MqttClientError> {
		fn missing_feature(what: &str) -> MqttClientError {
			MqttClientError::ConfigurationValue(format!(
				"{what} support is not compiled in — enable the corresponding \
				 feature on mqtt-typed-client"
			))
		}

		match &self.transport {
			| Transport::Tcp => Ok(rumqttc::Transport::Tcp),
			| Transport::Tls(tls) => {
				#[cfg(any(
					feature = "tls-rustls",
					feature = "tls-rustls-no-provider"
				))]
				{
					Ok(rumqttc::Transport::Tls(Self::rustls_configuration(tls)))
				}
				#[cfg(not(any(
					feature = "tls-rustls",
					feature = "tls-rustls-no-provider"
				)))]
				{
					let _ = tls;
					Err(missing_feature("TLS (`tls-rustls`)"))
				}
			}
			| Transport::Ws => {
				#[cfg(feature = "websocket")]
				{
					Ok(rumqttc::Transport::Ws)
				}
				#[cfg(not(feature = "websocket"))]
				{
					Err(missing_feature("WebSocket (`websocket`)"))
				}
			}
			| Transport::Wss(tls) => {
				#[cfg(all(
					feature = "websocket",
					any(
						feature = "tls-rustls",
						feature = "tls-rustls-no-provider"
					)
				))]
				{
					Ok(rumqttc::Transport::Wss(Self::rustls_configuration(tls)))
				}
				#[cfg(not(all(
					feature = "websocket",
					any(
						feature = "tls-rustls",
						feature = "tls-rustls-no-provider"
					)
				)))]
				{
					let _ = tls;
					Err(missing_feature(
						"TLS over WebSocket (`websocket` + `tls-rustls`)",
					))
				}
			}
		}
	}

	#[cfg(any(feature = "tls-rustls", feature = "tls-rustls-no-provider"))]
	fn rustls_configuration(tls: &TlsConfig) -> rumqttc::TlsConfiguration {
		match tls {
			| TlsConfig::Default => rumqttc::TlsConfiguration::default(),
			| TlsConfig::Rustls(config) => {
				rumqttc::TlsConfiguration::Rustls(config.config.clone())
			}
		}
	}
}

/// SEMVER-EXEMPT backend access (`unstable-backend-api` feature).
///
/// Everything in this module may change with any backend change, without a
/// major version bump. See [`ConnectionOptions::backend_tweak`].
#[cfg(feature = "unstable-backend-api")]
pub mod backend {
	/// The backend crate itself, version-matched to this library's build.
	pub use rumqttc;

	/// Mutable view of the raw backend options during facade conversion.
	///
	/// `#[non_exhaustive]` so a `V5` variant can arrive additively in 0.4.
	#[non_exhaustive]
	pub enum BackendOptions<'a> {
		/// MQTT 3.1.1 backend options
		V4(&'a mut rumqttc::MqttOptions),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn from_url_basic() {
		let opts =
			ConnectionOptions::from_url("mqtt://broker:1884?client_id=abc")
				.unwrap();
		assert_eq!(opts.host, "broker");
		assert_eq!(opts.port, 1884);
		assert_eq!(opts.client_id, "abc");
		assert!(matches!(opts.transport, Transport::Tcp));
		assert_eq!(opts.protocol, ProtocolVersion::V4);
	}

	#[test]
	fn from_url_default_ports_and_schemes() {
		let cases = [
			("tcp://h?client_id=c", 1883),
			("mqtts://h?client_id=c", 8883),
			("ssl://h?client_id=c", 8883),
			("ws://h?client_id=c", 8000),
			("wss://h?client_id=c", 8000), // rumqttc-compat quirk
		];
		for (url, port) in cases {
			assert_eq!(ConnectionOptions::from_url(url).unwrap().port, port);
		}
	}

	#[test]
	fn from_url_credentials_and_session() {
		let opts = ConnectionOptions::from_url(
			"mqtt://user:pass@h?client_id=c&clean_session=false&\
			 keep_alive_secs=30",
		)
		.unwrap();
		let creds = opts.credentials.unwrap();
		assert_eq!(creds.username, "user");
		assert_eq!(creds.password, "pass");
		assert_eq!(opts.session, SessionPolicy::Resume);
		assert_eq!(opts.keep_alive, Duration::from_secs(30));
	}

	#[test]
	fn from_url_protocol_param() {
		let opts =
			ConnectionOptions::from_url("mqtt://h?client_id=c&protocol=5")
				.unwrap();
		assert_eq!(opts.protocol, ProtocolVersion::V5);
		// ...but connecting with it is a clean 0.3 error:
		assert!(matches!(
			opts.to_backend_v4(),
			Err(MqttClientError::ConfigurationValue(msg)) if msg.contains("0.4")
		));
		assert!(matches!(
			ConnectionOptions::from_url("mqtt://h?client_id=c&protocol=3"),
			Err(UrlParseError::UnsupportedProtocol(_))
		));
	}

	#[test]
	fn from_url_rejects_moved_and_unknown_params() {
		assert!(matches!(
			ConnectionOptions::from_url(
				"mqtt://h?client_id=c&inflight_num=5"
			),
			Err(UrlParseError::UnsupportedParam(p)) if p == "inflight_num"
		));
		assert!(matches!(
			ConnectionOptions::from_url("mqtt://h?client_id=c&frobnicate=1"),
			Err(UrlParseError::UnknownParam(p)) if p == "frobnicate"
		));
		assert!(matches!(
			ConnectionOptions::from_url("mqtt://h"),
			Err(UrlParseError::MissingClientId)
		));
		assert!(matches!(
			ConnectionOptions::from_url("gopher://h?client_id=c"),
			Err(UrlParseError::Scheme(_))
		));
	}

	#[test]
	fn to_backend_validates_instead_of_panicking() {
		// sub-second keep-alive (rumqttc would assert)
		let mut opts = ConnectionOptions::new("c", "h", 1883);
		opts.keep_alive = Duration::from_millis(500);
		assert!(matches!(
			opts.to_backend_v4(),
			Err(MqttClientError::ConfigurationValue(_))
		));

		// zero keep-alive is valid (disables keep-alive)
		let mut opts = ConnectionOptions::new("c", "h", 1883);
		opts.keep_alive = Duration::ZERO;
		assert!(opts.to_backend_v4().is_ok());

		// Resume with empty client_id (rumqttc would assert)
		let mut opts = ConnectionOptions::new("", "h", 1883);
		opts.session = SessionPolicy::Resume;
		assert!(matches!(
			opts.to_backend_v4(),
			Err(MqttClientError::ConfigurationValue(_))
		));

		// ResumeFor is v5-only
		let mut opts = ConnectionOptions::new("c", "h", 1883);
		opts.session = SessionPolicy::ResumeFor(Duration::from_secs(60));
		assert!(matches!(
			opts.to_backend_v4(),
			Err(MqttClientError::ConfigurationValue(_))
		));
	}
}
