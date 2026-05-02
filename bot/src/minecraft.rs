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

pub async fn ping(port: u16) -> Option<ServerStatus> {
    let addr = format!("127.0.0.1:{port}");
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(&addr),
    )
    .await
    .ok()?
    .ok()?;

    let host = b"127.0.0.1";
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
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => {
                data.extend_from_slice(&buf[..n]);
                let opens = data.iter().filter(|&&b| b == b'{').count();
                let closes = data.iter().filter(|&&b| b == b'}').count();
                if opens > 0 && opens == closes {
                    break;
                }
            }
            Ok(Err(_)) => break,
        }
    }

    let start = data.iter().position(|&b| b == b'{')?;
    let json: serde_json::Value = serde_json::from_slice(&data[start..]).ok()?;
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
