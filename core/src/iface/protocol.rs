use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

const HEADER_SIZE: usize = 8;

#[derive(Debug, Deserialize)]
pub struct Request {
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub status: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Response {
            status: "ok".to_string(),
            data,
            error: String::new(),
        }
    }

    pub fn fail(error: &str) -> Self {
        Response {
            status: "error".to_string(),
            data: serde_json::Value::Null,
            error: error.to_string(),
        }
    }
}

pub fn read_request(stream: &mut UnixStream) -> std::io::Result<Request> {
    let mut header = [0u8; HEADER_SIZE];
    stream.read_exact(&mut header)?;
    let length = u64::from_be_bytes(header) as usize;

    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload)?;

    serde_json::from_slice(&payload)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

pub fn write_response(stream: &mut UnixStream, response: &Response) -> std::io::Result<()> {
    let payload = serde_json::to_vec(response)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let header = (payload.len() as u64).to_be_bytes();
    stream.write_all(&header)?;
    stream.write_all(&payload)?;
    stream.flush()
}
