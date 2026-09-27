use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AgentTaskCommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tuner_core::sha256_hex;

pub(crate) const UI_IPC_PROTOCOL: &str = "tuner-ui/v1";
const MAX_REQUEST_BYTES: usize = 1_000_000;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct UiIpcRequest {
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    pub action: String,
    #[serde(default)]
    pub agent_task: Option<AgentTaskCommand>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub semantic_id: Option<String>,
    #[serde(default)]
    pub axis_index: Option<usize>,
    #[serde(default)]
    pub table_key: Option<String>,
    #[serde(default)]
    pub row: Option<usize>,
    #[serde(default)]
    pub column: Option<usize>,
    #[serde(default)]
    pub row_end: Option<usize>,
    #[serde(default)]
    pub column_end: Option<usize>,
    #[serde(default)]
    pub byte_offset: Option<usize>,
    #[serde(default)]
    pub byte_length: Option<usize>,
    #[serde(default)]
    pub hex: Option<String>,
    #[serde(default)]
    pub display_format: Option<String>,
    #[serde(default)]
    pub endianness: Option<String>,
    #[serde(default)]
    pub decimal_places: Option<u8>,
    #[serde(default)]
    pub map_rows: Option<u16>,
    #[serde(default)]
    pub map_columns: Option<u16>,
    #[serde(default)]
    pub map_rows_min: Option<u16>,
    #[serde(default)]
    pub map_rows_max: Option<u16>,
    #[serde(default)]
    pub map_columns_min: Option<u16>,
    #[serde(default)]
    pub map_columns_max: Option<u16>,
    #[serde(default)]
    pub scan_all: Option<bool>,
    #[serde(default)]
    pub map_start_address: Option<String>,
    #[serde(default)]
    pub map_end_address: Option<String>,
    #[serde(default)]
    pub minimum_score: Option<u8>,
    #[serde(default)]
    pub skip_mapped: Option<bool>,
    #[serde(default)]
    pub map_every_byte: Option<bool>,
    #[serde(default)]
    pub map_zoom_percent: Option<u16>,
    #[serde(default)]
    pub candidate_pane_width: Option<u16>,
    #[serde(default)]
    pub map_position: Option<[i32; 2]>,
    #[serde(default)]
    pub map_size: Option<[u32; 2]>,
    #[serde(default)]
    pub candidate_index: Option<usize>,
    #[serde(default)]
    pub map_axis_index: Option<usize>,
    #[serde(default)]
    pub axis_role: Option<String>,
    #[serde(default)]
    pub color_mode: Option<String>,
    #[serde(default)]
    pub color_range: Option<[f64; 2]>,
    #[serde(default)]
    pub color_range_scope: Option<String>,
    #[serde(default)]
    pub color_low: Option<[u8; 4]>,
    #[serde(default)]
    pub color_middle: Option<[u8; 4]>,
    #[serde(default)]
    pub color_high: Option<[u8; 4]>,
    #[serde(default)]
    pub forward: Option<bool>,
    #[serde(default)]
    pub edit_value: Option<String>,
    #[serde(default)]
    pub clamp: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub changed_only: Option<bool>,
    #[serde(default)]
    pub selected_semantic_ids: Option<Vec<String>>,
    #[serde(default)]
    pub window_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<u64>,
    #[serde(default)]
    pub workspace_name: Option<String>,
}

#[cfg(test)]
impl UiIpcRequest {
    pub(crate) fn for_test(
        action: &str,
        semantic_id: &str,
        axis_index: Option<usize>,
        formula: Option<&str>,
    ) -> Self {
        Self {
            protocol: None,
            token: None,
            request_id: None,
            action: action.to_string(),
            agent_task: None,
            task_id: None,
            command: None,
            path: None,
            semantic_id: (!semantic_id.is_empty()).then(|| semantic_id.to_string()),
            axis_index,
            table_key: None,
            row: None,
            column: None,
            row_end: None,
            column_end: None,
            byte_offset: None,
            byte_length: None,
            hex: None,
            display_format: None,
            endianness: None,
            decimal_places: None,
            map_rows: None,
            map_columns: None,
            map_rows_min: None,
            map_rows_max: None,
            map_columns_min: None,
            map_columns_max: None,
            scan_all: None,
            map_start_address: None,
            map_end_address: None,
            minimum_score: None,
            skip_mapped: None,
            map_every_byte: None,
            map_zoom_percent: None,
            candidate_pane_width: None,
            map_position: None,
            map_size: None,
            candidate_index: None,
            map_axis_index: None,
            axis_role: None,
            color_mode: None,
            color_range: None,
            color_range_scope: None,
            color_low: None,
            color_middle: None,
            color_high: None,
            forward: None,
            edit_value: None,
            clamp: None,
            filter: None,
            query: None,
            formula: formula.map(str::to_string),
            changed_only: None,
            selected_semantic_ids: None,
            window_id: None,
            workspace_id: None,
            workspace_name: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct UiIpcEndpoint {
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub pid: u32,
    pub token: String,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Serialize)]
pub(crate) struct UiIpcResponse {
    pub protocol: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub action: String,
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl UiIpcResponse {
    pub(crate) fn ok(
        request: &UiIpcRequest,
        message: impl Into<String>,
        data: Option<Value>,
    ) -> Self {
        Self {
            protocol: UI_IPC_PROTOCOL,
            request_id: request.request_id.clone(),
            action: request.action.clone(),
            ok: true,
            message: message.into(),
            data,
        }
    }

    pub(crate) fn error(
        request: Option<&UiIpcRequest>,
        action: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            protocol: UI_IPC_PROTOCOL,
            request_id: request.and_then(|request| request.request_id.clone()),
            action: action.into(),
            ok: false,
            message: message.into(),
            data: None,
        }
    }
}

pub(crate) struct UiIpcCall {
    pub request: UiIpcRequest,
    responder: Sender<UiIpcResponse>,
}

impl UiIpcCall {
    pub(crate) fn respond(self, response: UiIpcResponse) {
        let _ = self.responder.send(response);
    }
}

pub(crate) struct UiIpcServer {
    endpoint: UiIpcEndpoint,
    receiver: Receiver<UiIpcCall>,
    stop: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
}

impl UiIpcServer {
    pub(crate) fn start(endpoint_path: PathBuf) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let token = make_token(port);
        let endpoint = UiIpcEndpoint {
            protocol: UI_IPC_PROTOCOL.to_string(),
            host: "127.0.0.1".to_string(),
            port,
            pid: process::id(),
            token: token.clone(),
            manifest_path: endpoint_path,
        };
        write_endpoint(&endpoint)?;

        let (sender, receiver) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let accept_stop = Arc::clone(&stop);
        let accept_token = token;
        let accept_thread = match thread::Builder::new()
            .name("tunernook-ui-ipc".to_string())
            .spawn(move || accept_loop(listener, sender, accept_token, accept_stop))
        {
            Ok(thread) => thread,
            Err(error) => {
                let _ = remove_endpoint_if_owned(&endpoint);
                return Err(error);
            }
        };

        Ok(Self {
            endpoint,
            receiver,
            stop,
            accept_thread: Some(accept_thread),
        })
    }

    pub(crate) fn try_recv(&self) -> Option<UiIpcCall> {
        self.receiver.try_recv().ok()
    }

    #[cfg(test)]
    pub(crate) fn endpoint(&self) -> &UiIpcEndpoint {
        &self.endpoint
    }

    pub(crate) fn public_summary(&self) -> Value {
        json!({
            "protocol": self.endpoint.protocol,
            "host": self.endpoint.host,
            "port": self.endpoint.port,
            "pid": self.endpoint.pid,
            "manifest_path": self.endpoint.manifest_path,
        })
    }
}

impl Drop for UiIpcServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
        let _ = remove_endpoint_if_owned(&self.endpoint);
    }
}

fn make_token(port: u16) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    sha256_hex(format!("{}:{port}:{timestamp}", process::id()).as_bytes())
}

fn normalized_request_line(line: &str) -> &str {
    line.strip_prefix('\u{feff}').unwrap_or(line)
}

fn write_endpoint(endpoint: &UiIpcEndpoint) -> io::Result<()> {
    if let Some(parent) = endpoint.manifest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_vec_pretty(endpoint).map_err(io::Error::other)?;
    fs::write(&endpoint.manifest_path, contents)
}

fn remove_endpoint_if_owned(endpoint: &UiIpcEndpoint) -> io::Result<()> {
    let owned = fs::read_to_string(&endpoint.manifest_path)
        .ok()
        .and_then(|contents| serde_json::from_str::<UiIpcEndpoint>(&contents).ok())
        .is_some_and(|current| current.token == endpoint.token && current.pid == endpoint.pid);
    if owned {
        fs::remove_file(&endpoint.manifest_path)?;
    }
    Ok(())
}

fn accept_loop(
    listener: TcpListener,
    sender: Sender<UiIpcCall>,
    token: String,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                let connection_sender = sender.clone();
                let connection_token = token.clone();
                let connection_stop = Arc::clone(&stop);
                let _ = thread::Builder::new()
                    .name("tunernook-ui-ipc-client".to_string())
                    .spawn(move || {
                        serve_connection(
                            stream,
                            connection_sender,
                            connection_token,
                            connection_stop,
                        )
                    });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn serve_connection(
    stream: TcpStream,
    sender: Sender<UiIpcCall>,
    token: String,
    stop: Arc<AtomicBool>,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(RESPONSE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(RESPONSE_TIMEOUT));
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        line.clear();
        let bytes_read = match reader.read_line(&mut line) {
            Ok(bytes_read) => bytes_read,
            Err(error) => {
                let response = UiIpcResponse::error(
                    None,
                    "request",
                    format!("Could not read JSONL request: {error}"),
                );
                let _ = write_response(reader.get_mut(), &response);
                break;
            }
        };
        if bytes_read == 0 {
            break;
        }
        let request_line = normalized_request_line(&line);
        let response = if request_line.len() > MAX_REQUEST_BYTES {
            UiIpcResponse::error(
                None,
                "request",
                format!("Request exceeds the {MAX_REQUEST_BYTES}-byte limit."),
            )
        } else {
            match serde_json::from_str::<UiIpcRequest>(request_line) {
                Ok(request) => authenticate_and_enqueue(request, &sender, &token, &stop),
                Err(error) => {
                    UiIpcResponse::error(None, "request", format!("Invalid JSONL request: {error}"))
                }
            }
        };
        let write_result = write_response(reader.get_mut(), &response);
        if write_result.is_err() {
            break;
        }
    }
}

fn authenticate_and_enqueue(
    request: UiIpcRequest,
    sender: &Sender<UiIpcCall>,
    token: &str,
    stop: &Arc<AtomicBool>,
) -> UiIpcResponse {
    if request.protocol.as_deref() != Some(UI_IPC_PROTOCOL) {
        return UiIpcResponse::error(
            Some(&request),
            request.action.clone(),
            format!("Unsupported protocol; expected {UI_IPC_PROTOCOL}."),
        );
    }
    if request.token.as_deref() != Some(token) {
        return UiIpcResponse::error(
            Some(&request),
            request.action.clone(),
            "Invalid local UI-control token.",
        );
    }
    let action = request.action.clone();
    let (responder, receiver) = mpsc::channel();
    if sender.send(UiIpcCall { request, responder }).is_err() {
        return UiIpcResponse::error(None, action, "The TunerNook UI is no longer available.");
    }
    loop {
        if stop.load(Ordering::Acquire) {
            return UiIpcResponse::error(None, action, "The TunerNook UI is shutting down.");
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(response) => return response,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return UiIpcResponse::error(None, action, "The UI request was cancelled.");
            }
        }
    }
}

fn write_response(writer: &mut TcpStream, response: &UiIpcResponse) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, response).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentTaskCommand;
    use std::net::TcpStream;
    use std::thread;

    #[test]
    fn parses_agent_task_start_without_affecting_legacy_requests() {
        let request: UiIpcRequest = serde_json::from_str(
            r#"{"action":"agent_task","agent_task":{"kind":"start","capability":"diagnostics","goal":"check mappings"}}"#,
        ).unwrap();
        assert!(matches!(
            request.agent_task,
            Some(AgentTaskCommand::Start { .. })
        ));
        assert!(serde_json::from_str::<UiIpcRequest>(r#"{"action":"ping"}"#).is_ok());
    }

    #[test]
    fn parses_task_id_for_raw_byte_reads() {
        let request: UiIpcRequest = serde_json::from_str(
            r#"{"action":"read_bin_bytes","task_id":"task-1","byte_offset":0,"byte_length":1}"#,
        )
        .unwrap();
        assert_eq!(request.task_id.as_deref(), Some("task-1"));
    }

    #[test]
    fn local_server_authenticates_and_round_trips_a_request() {
        let manifest = std::env::temp_dir().join(format!(
            "tunernook-ui-ipc-test-{}-{}.json",
            process::id(),
            make_token(0)
        ));
        let server = UiIpcServer::start(manifest.clone()).unwrap();
        let endpoint = server.endpoint().clone();
        let mut stream = loop {
            match TcpStream::connect((endpoint.host.as_str(), endpoint.port)) {
                Ok(stream) => break stream,
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        };
        let request = json!({
            "protocol": UI_IPC_PROTOCOL,
            "token": endpoint.token,
            "request_id": "test-1",
            "action": "ping"
        });
        writeln!(stream, "{request}").unwrap();
        stream.flush().unwrap();

        let call = loop {
            if let Some(call) = server.try_recv() {
                break call;
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(call.request.action, "ping");
        call.respond(UiIpcResponse::ok(
            &UiIpcRequest {
                protocol: Some(UI_IPC_PROTOCOL.to_string()),
                token: None,
                request_id: Some("test-1".to_string()),
                action: "ping".to_string(),
                agent_task: None,
                task_id: None,
                command: None,
                path: None,
                semantic_id: None,
                axis_index: None,
                table_key: None,
                row: None,
                column: None,
                row_end: None,
                column_end: None,
                byte_offset: None,
                byte_length: None,
                hex: None,
                display_format: None,
                endianness: None,
                decimal_places: None,
                map_rows: None,
                map_columns: None,
                map_rows_min: None,
                map_rows_max: None,
                map_columns_min: None,
                map_columns_max: None,
                scan_all: None,
                map_start_address: None,
                map_end_address: None,
                minimum_score: None,
                skip_mapped: None,
                map_every_byte: None,
                map_zoom_percent: None,
                candidate_pane_width: None,
                map_position: None,
                map_size: None,
                candidate_index: None,
                map_axis_index: None,
                axis_role: None,
                color_mode: None,
                color_range: None,
                color_range_scope: None,
                color_low: None,
                color_middle: None,
                color_high: None,
                forward: None,
                edit_value: None,
                clamp: None,
                filter: None,
                query: None,
                formula: None,
                changed_only: None,
                selected_semantic_ids: None,
                window_id: None,
                workspace_id: None,
                workspace_name: None,
            },
            "pong",
            None,
        ));

        let mut response_line = String::new();
        BufReader::new(stream)
            .read_line(&mut response_line)
            .unwrap();
        let response: Value = serde_json::from_str(&response_line).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["message"], "pong");
        drop(server);
        assert!(!manifest.exists());
    }

    #[test]
    fn jsonl_parser_accepts_a_windows_utf8_bom() {
        let request: UiIpcRequest =
            serde_json::from_str(normalized_request_line("{\"action\":\"ping\"}"))
                .expect("plain JSON should parse");
        assert_eq!(request.action, "ping");
        let bom_request: UiIpcRequest =
            serde_json::from_str(normalized_request_line("\u{feff}{\"action\":\"ping\"}"))
                .expect("the normalized JSONL line should parse");
        assert_eq!(bom_request.action, "ping");
    }
}
