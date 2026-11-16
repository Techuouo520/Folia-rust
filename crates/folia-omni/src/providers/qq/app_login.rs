// QQ Music's shared app QR channel over MQTT 5/WSS, following qq-music-api 3.3.3.
use std::time::{Duration, Instant};

use bytes::BytesMut;
use futures_util::{SinkExt, StreamExt};
use rumqttc::v5::mqttbytes::v5::{
    Connect, ConnectProperties, ConnectReturnCode, Filter, Packet, PingReq, PubAck, Publish, Subscribe,
    SubscribeProperties, SubscribeReasonCode,
};
use rumqttc::v5::mqttbytes::{Error as PacketError, QoS};
use serde_json::Value;
use tokio_tungstenite::tungstenite::{
    Error as SocketError, Message, client::IntoClientRequest, error::TlsError, protocol::WebSocketConfig,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{LoginState, OmniError};

const MAX_PACKET: u32 = 64 * 1024;
const HANDSHAKE: Duration = Duration::from_secs(20);
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub enum Poll {
    Status(LoginState),
    Authorized(Value),
}

pub struct Session {
    connection: Connection,
    topic: String,
    expires: Instant,
    ping_at: Instant,
    state: LoginState,
    authorization: Option<Value>,
}

impl Session {
    pub async fn open(id: &str, expires: Instant) -> Result<Self, OmniError> {
        tokio::time::timeout(HANDSHAKE, Self::subscribe(id, expires))
            .await
            .map_err(|_| OmniError::Response("QQ sign-in subscription timed out".into()))?
    }

    async fn subscribe(id: &str, expires: Instant) -> Result<Self, OmniError> {
        let connection = Connection::open(id).await?;
        let topic = format!("management.qrcode_login/{id}");
        let mut session = Self {
            connection,
            topic,
            expires,
            ping_at: Instant::now(),
            state: LoginState::Waiting,
            authorization: None,
        };
        session
            .connection
            .send(Packet::Subscribe(Subscribe {
                pkid: 1,
                filters: vec![Filter::new(&session.topic, QoS::AtMostOnce)],
                properties: Some(SubscribeProperties {
                    user_properties: vec![
                        ("authorization".into(), "tmelogin".into()),
                        ("pubsub".into(), "unicast".into()),
                    ],
                    id: None,
                }),
            }))
            .await?;
        loop {
            match session.connection.next().await? {
                Packet::SubAck(ack)
                    if ack.pkid == 1 && ack.return_codes == [SubscribeReasonCode::Success(QoS::AtMostOnce)] =>
                {
                    return Ok(session);
                }
                Packet::SubAck(_) => return Err(OmniError::Response("QQ sign-in subscription rejected".into())),
                Packet::Publish(event) => session.accept(event).await?,
                Packet::Disconnect(_) => return Err(OmniError::Response("QQ sign-in connection closed".into())),
                _ => {}
            }
        }
    }

    pub async fn poll(&mut self) -> Result<Poll, OmniError> {
        if Instant::now() >= self.expires {
            return Ok(Poll::Status(LoginState::Expired));
        }
        if let Some(payload) = &self.authorization {
            return Ok(Poll::Authorized(payload.clone()));
        }
        if self.ping_at.elapsed() >= Duration::from_secs(30) {
            self.connection.send(Packet::PingReq(PingReq)).await?;
            self.ping_at = Instant::now();
        }
        let until = tokio::time::Instant::now() + Duration::from_secs(1);
        while let Ok(packet) = tokio::time::timeout_at(until, self.connection.next()).await {
            match packet? {
                Packet::Publish(event) => {
                    self.accept(event).await?;
                    if let Some(payload) = &self.authorization {
                        return Ok(Poll::Authorized(payload.clone()));
                    }
                    return Ok(Poll::Status(self.state.clone()));
                }
                Packet::Disconnect(_) => return Err(OmniError::Response("QQ sign-in connection closed".into())),
                _ => {}
            }
        }
        Ok(Poll::Status(self.state.clone()))
    }

    async fn accept(&mut self, event: Publish) -> Result<(), OmniError> {
        if event.topic.as_ref() != self.topic.as_bytes() {
            return Err(OmniError::Response("QQ sign-in event topic differs".into()));
        }
        match event.qos {
            QoS::AtLeastOnce => self.connection.send(Packet::PubAck(PubAck::new(event.pkid, None))).await?,
            QoS::ExactlyOnce => return Err(OmniError::Response("QQ sign-in event QoS differs".into())),
            QoS::AtMostOnce => {}
        }
        let kind = event.properties.as_ref().and_then(|properties| {
            properties.user_properties.iter().find(|(key, _)| key == "type").map(|(_, value)| value.as_str())
        });
        match kind {
            Some("scanned") => self.state = LoginState::Scanned,
            Some("cookies") => {
                let payload = serde_json::from_slice(&event.payload)
                    .map_err(|_| OmniError::Response("QQ sign-in event payload is malformed".into()))?;
                self.authorization = Some(payload);
            }
            Some("timeout") => self.state = LoginState::Expired,
            Some("canceled" | "loginFailed") => self.state = LoginState::Failed { code: -1, message: None },
            _ => {}
        }
        Ok(())
    }
}

struct Connection {
    socket: Socket,
    pending: BytesMut,
}

impl Connection {
    async fn open(id: &str) -> Result<Self, OmniError> {
        let tls = tls_config()?;
        let mut path = "/ws/handshake".to_owned();
        for redirect in 0..=3 {
            let mut request = format!("wss://mu.y.qq.com{path}").into_client_request().map_err(socket_error)?;
            request.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                "mqtt".parse().map_err(|_| OmniError::Response("QQ sign-in protocol is invalid".into()))?,
            );
            let mut config = WebSocketConfig::default();
            config.max_message_size = Some(MAX_PACKET as usize);
            config.max_frame_size = Some(MAX_PACKET as usize);
            let (socket, _) = tokio_tungstenite::connect_async_tls_with_config(
                request,
                Some(config),
                false,
                Some(tokio_tungstenite::Connector::Rustls(tls.clone())),
            )
            .await
            .map_err(socket_error)?;
            let mut connection = Self { socket, pending: BytesMut::new() };
            connection.send(connect_packet(id)).await?;
            let Packet::ConnAck(ack) = connection.next().await? else {
                return Err(OmniError::Response("QQ sign-in expected CONNACK".into()));
            };
            if ack.code == ConnectReturnCode::Success {
                return Ok(connection);
            }
            if redirect == 3
                || !matches!(ack.code, ConnectReturnCode::UseAnotherServer | ConnectReturnCode::ServerMoved)
            {
                return Err(OmniError::Response(format!("QQ sign-in connection rejected: {:?}", ack.code)));
            }
            let reference = ack
                .properties
                .and_then(|properties| properties.server_reference)
                .filter(|value| {
                    !value.is_empty()
                        && value.len() <= 256
                        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b".:-_".contains(&b))
                })
                .ok_or_else(|| OmniError::Response("QQ sign-in redirect is invalid".into()))?;
            let (prefix, last) = path.rsplit_once('/').unwrap_or(("", &path));
            path = if last.contains(':') { format!("{prefix}/{reference}") } else { format!("{path}/{reference}") };
        }
        Err(OmniError::Response("QQ sign-in redirect limit exceeded".into()))
    }

    async fn send(&mut self, packet: Packet) -> Result<(), OmniError> {
        let mut bytes = BytesMut::new();
        packet.write(&mut bytes, Some(MAX_PACKET))?;
        tokio::time::timeout(HANDSHAKE, self.socket.send(Message::Binary(bytes.freeze())))
            .await
            .map_err(|_| OmniError::Response("QQ sign-in send timed out".into()))?
            .map_err(socket_error)
    }

    async fn next(&mut self) -> Result<Packet, OmniError> {
        loop {
            match Packet::read(&mut self.pending, Some(MAX_PACKET)) {
                Ok(packet) => return Ok(packet),
                Err(PacketError::InsufficientBytes(_)) => {}
                Err(err) => return Err(err.into()),
            }
            match self.socket.next().await.transpose().map_err(socket_error)? {
                Some(Message::Binary(data)) if self.pending.len() + data.len() <= MAX_PACKET as usize => {
                    self.pending.extend_from_slice(&data)
                }
                Some(Message::Ping(_) | Message::Pong(_)) => {}
                _ => return Err(OmniError::Response("QQ sign-in connection ended or sent invalid data".into())),
            }
        }
    }
}

fn connect_packet(id: &str) -> Packet {
    let properties = ConnectProperties {
        authentication_method: Some("pass".into()),
        user_properties: vec![
            ("tmeAppID".into(), "qqmusic".into()),
            ("business".into(), "management".into()),
            ("hashTag".into(), id.into()),
            ("clientTag".into(), "management.user".into()),
            ("userID".into(), id.into()),
        ],
        ..ConnectProperties::new()
    };
    Packet::Connect(
        Connect {
            keep_alive: 45,
            client_id: format!("{}{}", super::musicu::now_millis(), super::musicu::random_digits(4)),
            clean_start: true,
            properties: Some(properties),
        },
        None,
        None,
    )
}

fn socket_error(error: SocketError) -> OmniError {
    OmniError::QqSocket(Box::new(error))
}

fn tls_config() -> Result<std::sync::Arc<rustls::ClientConfig>, OmniError> {
    let certificates = rustls_native_certs::load_native_certs();
    for error in certificates.errors {
        tracing::warn!(
            error = &error as &dyn std::error::Error,
            "QQ sign-in certificate store entry could not be loaded"
        );
    }
    let mut roots = rustls::RootCertStore::empty();
    let (accepted, rejected) = roots.add_parsable_certificates(certificates.certs);
    if rejected > 0 {
        tracing::warn!(rejected, "QQ sign-in certificate store contained invalid entries");
    }
    if accepted == 0 {
        return Err(OmniError::Response("QQ sign-in certificate store is empty".into()));
    }
    let config =
        rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|error| socket_error(SocketError::Tls(TlsError::Rustls(Box::new(error)))))?
            .with_root_certificates(roots)
            .with_no_client_auth();
    Ok(std::sync::Arc::new(config))
}
