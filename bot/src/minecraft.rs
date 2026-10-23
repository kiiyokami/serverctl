use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn varint(mut n: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (n & 0x7F) as u8;
        n >>= 7;
        if n != 0 {
            out.push(b | 0x80);
        } else {
            out.push(b);
            break;
        }
    }
    out
}

pub struct ServerStatus {
    pub online: u32,
    pub max: u32,
}

/// Server List Ping via the server's Service. The bot pod isn't on the host
/// network, so 127.0.0.1:<nodePort> would hit the bot's own pod.
pub async fn ping(server: &str) -> Option<ServerStatus> {
    let host = format!("{server}.games.svc.cluster.local");
    let port: u16 = 25565;
    let addr = format!("{host}:{port}");
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(&addr),
    )
    .await
    .ok()?
    .ok()?;

    let host = host.as_bytes();
    let mut handshake = Vec::new();
    handshake.extend(varint(0x00));
    handshake.extend(varint(765));
    handshake.extend(varint(host.len() as u32));
    handshake.extend_from_slice(host);
    handshake.extend_from_slice(&port.to_be_bytes());
    handshake.extend(varint(1));

    let mut packet = varint(handshake.len() as u32);
    packet.extend(handshake);
    packet.extend_from_slice(&[0x01, 0x00]);

    stream.write_all(&packet).await.ok()?;

    let mut buf = vec![0u8; 4096];
    let mut data = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, stream.read(&mut buf)).await {
            Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
            Ok(Ok(n)) => {
                data.extend_from_slice(&buf[..n]);
                if status_json(&data).is_some() {
                    break;
                }
            }
        }
    }
    parse_status(&data)
}

fn read_varint(buf: &[u8]) -> Option<(u32, usize)> {
    let mut n = 0u32;
    for (i, &byte) in buf.iter().enumerate().take(5) {
        n |= u32::from(byte & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((n, i + 1));
        }
    }
    None
}

/// The JSON of a status response, once `data` holds the whole packet:
/// length, packet ID, then a length-prefixed string.
fn status_json(data: &[u8]) -> Option<&[u8]> {
    let (len, a) = read_varint(data)?;
    let packet = data.get(a..a.checked_add(len as usize)?)?;
    let (_id, b) = read_varint(packet)?;
    let (json_len, c) = read_varint(&packet[b..])?;
    packet.get(b + c..(b + c).checked_add(json_len as usize)?)
}

fn parse_status(data: &[u8]) -> Option<ServerStatus> {
    let json: serde_json::Value = serde_json::from_slice(status_json(data)?).ok()?;
    let online = json["players"]["online"].as_u64()? as u32;
    let max = json["players"]["max"].as_u64().unwrap_or(20) as u32;
    Some(ServerStatus { online, max })
}

pub fn format_uptime(secs: i64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(json: &str) -> Vec<u8> {
        let mut body = varint(0x00);
        body.extend(varint(json.len() as u32));
        body.extend_from_slice(json.as_bytes());
        let mut out = varint(body.len() as u32);
        out.extend(body);
        out
    }

    #[test]
    fn reads_players_even_with_braces_in_motd() {
        let data = packet(r#"{"description":{"text":"{SMP"},"players":{"online":3,"max":10}}"#);
        let s = parse_status(&data).unwrap();
        assert_eq!((s.online, s.max), (3, 10));
    }

    #[test]
    fn waits_for_the_whole_packet() {
        let data = packet(&format!(r#"{{"favicon":"{}","players":{{"online":1,"max":5}}}}"#, "a".repeat(20_000)));
        assert!(status_json(&data[..4096]).is_none());
        assert_eq!(parse_status(&data).unwrap().online, 1);
    }
}
