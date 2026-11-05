//! Minimal MQTT 5 over WSS transport for QQ Music's QR login push channel.
use crate::{
    auth::Credentials,
    error::{AppError, Result},
    login::QrStatus,
    qq::QqMusic,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};

pub struct MobileSession {
    state: Arc<Mutex<Result<QrStatus>>>,
    task: tokio::task::AbortHandle,
}
impl Drop for MobileSession {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl MobileSession {
    pub fn start(id: String, qq: QqMusic) -> Self {
        let state = Arc::new(Mutex::new(Ok(QrStatus::Waiting)));
        let target = state.clone();
        let task = tokio::spawn(async move {
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(180), run(&id, &qq, &target))
                    .await;
            let value = match result {
                Ok(result) => result,
                Err(_) => Ok(QrStatus::Expired),
            };
            if let Ok(mut state) = target.lock() {
                *state = value;
            }
        });
        Self {
            state,
            task: task.abort_handle(),
        }
    }
    pub fn poll(&self) -> Result<QrStatus> {
        self.state
            .lock()
            .map_err(|_| AppError::new("QR_LOGIN_FAILED", "扫码推送状态异常。"))?
            .clone()
    }
}
fn protocol() -> AppError {
    AppError::new(
        "QR_PROTOCOL_ERROR",
        "QQ 音乐扫码推送格式无效，请刷新二维码。",
    )
}
fn network() -> AppError {
    AppError::new(
        "QR_NETWORK_ERROR",
        "无法连接 QQ 音乐扫码推送服务，请检查网络。",
    )
}
fn utf8(value: &str) -> Vec<u8> {
    let mut out = (value.len() as u16).to_be_bytes().to_vec();
    out.extend(value.as_bytes());
    out
}
fn variable(mut n: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n % 128) as u8;
        n /= 128;
        out.push(byte | if n > 0 { 128 } else { 0 });
        if n == 0 {
            return out;
        }
    }
}
fn packet(header: u8, body: Vec<u8>) -> Message {
    let mut out = vec![header];
    out.extend(variable(body.len()));
    out.extend(body);
    Message::Binary(out.into())
}
fn user_properties(pairs: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in pairs {
        out.push(0x26);
        out.extend(utf8(k));
        out.extend(utf8(v));
    }
    out
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or_else(protocol)?;
        let value = self.bytes.get(self.at..end).ok_or_else(protocol)?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn short(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(
            self.take(2)?.try_into().expect("2 bytes"),
        ))
    }
    fn string(&mut self) -> Result<String> {
        let n = self.short()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| protocol())
    }
    fn variable(&mut self) -> Result<usize> {
        let mut n = 0;
        for i in 0..4 {
            let b = self.byte()?;
            n += ((b & 127) as usize) << (7 * i);
            if b & 128 == 0 {
                return Ok(n);
            }
        }
        Err(protocol())
    }
    fn properties(&mut self) -> Result<std::collections::HashMap<String, String>> {
        let len = self.variable()?;
        let data = self.take(len)?;
        let mut r = Reader { bytes: data, at: 0 };
        let mut result = std::collections::HashMap::new();
        while r.at < data.len() {
            match r.byte()? {
                0x26 => {
                    let key = r.string()?;
                    result.insert(key, r.string()?);
                }
                0x1c => {
                    result.insert("server".into(), r.string()?);
                }
                0x01 | 0x17 | 0x19 | 0x24 | 0x25 | 0x28 | 0x29 | 0x2a => {
                    r.take(1)?;
                }
                0x13 | 0x21 | 0x22 | 0x23 => {
                    r.take(2)?;
                }
                0x02 | 0x11 | 0x18 | 0x27 => {
                    r.take(4)?;
                }
                0x0b => {
                    r.variable()?;
                }
                0x03 | 0x08 | 0x09 | 0x12 | 0x15 | 0x16 | 0x1a | 0x1f => {
                    let n = r.short()? as usize;
                    r.take(n)?;
                }
                _ => return Err(protocol()),
            }
        }
        Ok(result)
    }
}
fn frame(bytes: &mut Vec<u8>) -> Result<Option<(u8, Vec<u8>)>> {
    if bytes.len() < 2 {
        return Ok(None);
    }
    let mut len = 0usize;
    let mut header = 0;
    for i in 0..4 {
        let Some(&b) = bytes.get(i + 1) else {
            return Ok(None);
        };
        len += ((b & 127) as usize) << (7 * i);
        if b & 128 == 0 {
            header = i + 2;
            break;
        }
    }
    if header == 0 || len > 2 * 1024 * 1024 {
        return Err(protocol());
    }
    if bytes.len() < header + len {
        return Ok(None);
    }
    let kind = bytes[0];
    let body = bytes[header..header + len].to_vec();
    bytes.drain(..header + len);
    Ok(Some((kind, body)))
}
async fn run(id: &str, qq: &QqMusic, state: &Arc<Mutex<Result<QrStatus>>>) -> Result<QrStatus> {
    let mut path = "/ws/handshake".to_owned();
    for attempt in 0..4 {
        let mut request = format!("wss://mu.y.qq.com{path}")
            .into_client_request()
            .map_err(|_| protocol())?;
        request
            .headers_mut()
            .insert("Origin", "https://y.qq.com".parse().expect("header"));
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", "mqtt".parse().expect("header"));
        let (mut socket, _) =
            tokio::time::timeout(std::time::Duration::from_secs(20), connect_async(request))
                .await
                .map_err(|_| network())?
                .map_err(|_| network())?;
        let mut properties = vec![0x15];
        properties.extend(utf8("pass"));
        properties.extend(user_properties(&[
            ("tmeAppID", "qqmusic"),
            ("business", "management"),
            ("hashTag", id),
            ("clientTag", "management.user"),
            ("userID", id),
        ]));
        let mut connect = utf8("MQTT");
        connect.extend([5, 2, 0, 45]);
        connect.extend(variable(properties.len()));
        connect.extend(properties);
        connect.extend(utf8(&uuid::Uuid::new_v4().to_string()));
        socket
            .send(packet(0x10, connect))
            .await
            .map_err(|_| network())?;
        let mut bytes = Vec::new();
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(15));
        let handshake = tokio::time::sleep(std::time::Duration::from_secs(20));
        tokio::pin!(handshake);
        let mut connected = false;
        let mut redirect = None;
        loop {
            tokio::select! {
             _=&mut handshake, if !connected => return Err(network()),
             _=heartbeat.tick(), if connected => {socket.send(Message::Binary(vec![0xc0,0].into())).await.map_err(|_|network())?;},
             incoming=socket.next()=>{
              match incoming.ok_or_else(network)?.map_err(|_|network())?{Message::Binary(data)=>bytes.extend(data),Message::Ping(data)=>{socket.send(Message::Pong(data)).await.map_err(|_|network())?;},Message::Close(_)=>return Err(network()),_=>{}}
             }
            }
            while let Some((header, body)) = frame(&mut bytes)? {
                let mut r = Reader {
                    bytes: &body,
                    at: 0,
                };
                match header >> 4 {
                    2 => {
                        r.byte()?;
                        let code = r.byte()?;
                        let props = r.properties()?;
                        if code == 0 {
                            connected = true;
                            let props = user_properties(&[
                                ("authorization", "tmelogin"),
                                ("pubsub", "unicast"),
                            ]);
                            let mut subscribe = vec![0, 1];
                            subscribe.extend(variable(props.len()));
                            subscribe.extend(props);
                            subscribe.extend(utf8(&format!("management.qrcode_login/{id}")));
                            subscribe.push(0);
                            socket
                                .send(packet(0x82, subscribe))
                                .await
                                .map_err(|_| network())?;
                        } else if matches!(code, 0x9c | 0x9d) && attempt < 3 {
                            let server = props.get("server").ok_or_else(protocol)?;
                            if !server
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b".:-_".contains(&b))
                            {
                                return Err(protocol());
                            }
                            redirect = Some(format!("/ws/handshake/{server}"));
                            break;
                        } else {
                            return Err(AppError::new(
                                "QR_LOGIN_FAILED",
                                format!("扫码推送服务拒绝连接（{code}）。"),
                            ));
                        }
                    }
                    3 => {
                        if r.string()? != format!("management.qrcode_login/{id}") {
                            return Err(protocol());
                        }
                        let qos = (header >> 1) & 3;
                        let packet_id = if qos > 0 { Some(r.short()?) } else { None };
                        let props = r.properties()?;
                        let kind = props.get("type").ok_or_else(protocol)?;
                        if qos == 1 {
                            let mut ack = packet_id.expect("QoS1 packet id").to_be_bytes().to_vec();
                            ack.extend([0, 0]);
                            socket
                                .send(packet(0x40, ack))
                                .await
                                .map_err(|_| network())?;
                        }
                        match kind.as_str() {
                            "scanned" => {
                                *state.lock().map_err(|_| protocol())? = Ok(QrStatus::Scanned);
                            }
                            "canceled" => return Ok(QrStatus::Refused),
                            "timeout" => return Ok(QrStatus::Expired),
                            "loginFailed" => {
                                return Err(AppError::new(
                                    "QR_LOGIN_FAILED",
                                    "手机 QQ 音乐未完成登录授权。",
                                ))
                            }
                            "cookies" => {
                                let payload: Value = serde_json::from_slice(&body[r.at..])?;
                                let uin = payload
                                    .pointer("/cookies/qqmusic_uin/value")
                                    .and_then(|v| {
                                        v.as_str()
                                            .map(str::to_string)
                                            .or_else(|| v.as_u64().map(|n| n.to_string()))
                                    })
                                    .ok_or_else(protocol)?;
                                let key = payload
                                    .pointer("/cookies/qqmusic_key/value")
                                    .and_then(Value::as_str)
                                    .ok_or_else(protocol)?;
                                let data=qq.rpc(None,json!({"ct":23,"cv":0,"tmeLoginType":6}),"music.login.LoginServer","Login",json!({"musicid":uin.parse::<u64>().map_err(|_|protocol())?,"qrCodeID":id,"token":key})).await?;
                                return Ok(QrStatus::Authorized(Credentials::from_login(data, 6)?));
                            }
                            _ => {}
                        }
                    }
                    9 => {
                        r.short()?;
                        r.properties()?;
                        if r.byte()? >= 0x80 {
                            return Err(AppError::new("QR_LOGIN_FAILED", "扫码状态订阅被拒绝。"));
                        }
                    }
                    13 => {}
                    14 => return Err(network()),
                    _ => return Err(protocol()),
                }
            }
            if redirect.is_some() {
                break;
            }
        }
        if let Some(next) = redirect {
            socket.close(None).await.map_err(|_| network())?;
            path = next;
        }
    }
    Err(network())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mqtt_frames_can_arrive_in_chunks() {
        let mut bytes = vec![0x20, 3, 0];
        assert!(frame(&mut bytes).unwrap().is_none());
        bytes.extend([0, 0]);
        assert_eq!(frame(&mut bytes).unwrap().unwrap().0, 0x20);
        assert!(bytes.is_empty());
    }
    #[test]
    fn rejects_incomplete_properties() {
        let mut r = Reader {
            bytes: &[3, 0x26, 0],
            at: 0,
        };
        assert!(r.properties().is_err());
    }
}
