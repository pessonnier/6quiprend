use crate::board::{parse_move_payload, parse_name_payload, BoardError, BoardState};

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    mpsc::{self, RecvTimeoutError, Sender},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const STYLES_CSS: &str = include_str!("../web/styles.css");
const PLAYER_COOKIE_NAME: &str = "six_qui_prend_player";

pub fn run(listen_address: &str) -> io::Result<()> {
    let listener = TcpListener::bind(listen_address)?;
    let state = Arc::new(ApplicationState::new());

    println!("6 qui prend server listening on http://{listen_address}");

    for connection in listener.incoming() {
        let stream = connection?;
        let state = Arc::clone(&state);

        thread::spawn(move || {
            if let Err(error) = handle_connection(stream, state) {
                eprintln!("connection error: {error}");
            }
        });
    }

    Ok(())
}

struct ApplicationState {
    board: Mutex<BoardState>,
    subscribers: Mutex<Vec<Subscriber>>,
}

struct Subscriber {
    player_id: String,
    sender: Sender<String>,
}

struct PlayerSession {
    player_id: String,
    was_created: bool,
}

impl ApplicationState {
    fn new() -> Self {
        Self {
            board: Mutex::new(BoardState::new()),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    fn ensure_player(&self, request: &HttpRequest) -> PlayerSession {
        let cookie_player_id = request.cookie_value(PLAYER_COOKIE_NAME);
        let mut board = self.board.lock().expect("board mutex poisoned");
        let known_player = cookie_player_id
            .as_deref()
            .is_some_and(|player_id| board.has_player(player_id));
        let player_id = board.ensure_player(cookie_player_id.as_deref());

        PlayerSession {
            player_id,
            was_created: !known_player,
        }
    }

    fn view_for(&self, player_id: &str) -> String {
        self.board
            .lock()
            .expect("board mutex poisoned")
            .view_for(player_id)
    }
}

fn handle_connection(mut stream: TcpStream, state: Arc<ApplicationState>) -> io::Result<()> {
    let Some(request) = HttpRequest::read_from(&mut stream)? else {
        return Ok(());
    };

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => write_response(&mut stream, HttpResponse::html(INDEX_HTML.to_string())),
        ("GET", "/app.js") => {
            write_response(&mut stream, HttpResponse::javascript(APP_JS.to_string()))
        }
        ("GET", "/styles.css") => {
            write_response(&mut stream, HttpResponse::css(STYLES_CSS.to_string()))
        }
        ("GET", "/api/session") => handle_session(&mut stream, request, state),
        ("GET", "/api/board") => handle_board_view(&mut stream, request, state),
        ("GET", "/api/events") => handle_event_stream(stream, request, state),
        ("POST", "/api/session/name") => handle_rename_player(&mut stream, request, state),
        ("POST", path) if is_card_play_path(path) => handle_card_play(&mut stream, request, state),
        ("POST", path) if is_card_take_path(path) => handle_card_take(&mut stream, request, state),
        ("POST", path) if is_card_move_path(path) => handle_card_move(&mut stream, request, state),
        _ => write_response(
            &mut stream,
            HttpResponse::text(404, "Not found".to_string()),
        ),
    }
}

fn handle_session(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);

    if session.was_created {
        broadcast_views(&state);
    }

    let view = state.view_for(&session.player_id);

    write_response(
        stream,
        HttpResponse::json(200, view).with_player_cookie(&session.player_id),
    )
}

fn handle_board_view(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);

    if session.was_created {
        broadcast_views(&state);
    }

    let view = state.view_for(&session.player_id);

    write_response(
        stream,
        HttpResponse::json(200, view).with_player_cookie(&session.player_id),
    )
}

fn handle_rename_player(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);
    let player_id = session.player_id;

    if session.was_created {
        broadcast_views(&state);
    }

    let response = match parse_name_payload(&request.body) {
        Ok(name) => {
            let result = {
                let mut board = state.board.lock().expect("board mutex poisoned");
                board.rename_player(&player_id, &name)
            };

            match result {
                Ok(()) => {
                    broadcast_views(&state);
                    HttpResponse::json(200, state.view_for(&player_id))
                }
                Err(error) => response_for_board_error(error),
            }
        }
        Err(error) => response_for_board_error(error),
    };

    write_response(stream, response.with_player_cookie(&player_id))
}

fn handle_card_play(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);
    let player_id = session.player_id;

    if session.was_created {
        broadcast_views(&state);
    }

    let card_id = request
        .path
        .trim_start_matches("/api/cards/")
        .trim_end_matches("/play")
        .to_string();
    let response = match parse_move_payload(&request.body) {
        Ok((x, y)) => {
            let result = {
                let mut board = state.board.lock().expect("board mutex poisoned");
                board.play_card_from_hand(&player_id, &card_id, x, y)
            };
            response_after_mutation(result, &state, &player_id)
        }
        Err(error) => response_for_board_error(error),
    };

    write_response(stream, response.with_player_cookie(&player_id))
}

fn handle_card_take(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);
    let player_id = session.player_id;

    if session.was_created {
        broadcast_views(&state);
    }

    let card_id = request
        .path
        .trim_start_matches("/api/cards/")
        .trim_end_matches("/take")
        .to_string();
    let result = {
        let mut board = state.board.lock().expect("board mutex poisoned");
        board.take_card_from_board(&player_id, &card_id)
    };
    let response = response_after_mutation(result, &state, &player_id);

    write_response(stream, response.with_player_cookie(&player_id))
}

fn handle_card_move(
    stream: &mut TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);
    let player_id = session.player_id;

    if session.was_created {
        broadcast_views(&state);
    }

    let card_id = request
        .path
        .trim_start_matches("/api/cards/")
        .trim_end_matches("/move")
        .to_string();
    let response = match parse_move_payload(&request.body) {
        Ok((x, y)) => {
            let result = {
                let mut board = state.board.lock().expect("board mutex poisoned");
                board.move_board_card(&card_id, x, y)
            };
            response_after_mutation(result, &state, &player_id)
        }
        Err(error) => response_for_board_error(error),
    };

    write_response(stream, response.with_player_cookie(&player_id))
}

fn response_after_mutation(
    result: Result<(), BoardError>,
    state: &Arc<ApplicationState>,
    player_id: &str,
) -> HttpResponse {
    match result {
        Ok(()) => {
            broadcast_views(state);
            HttpResponse::json(200, state.view_for(player_id))
        }
        Err(error) => response_for_board_error(error),
    }
}

fn response_for_board_error(error: BoardError) -> HttpResponse {
    match error {
        BoardError::CardNotFound => HttpResponse::text(404, "Unknown card".to_string()),
        BoardError::PlayerNotFound => HttpResponse::text(404, "Unknown player".to_string()),
        BoardError::InvalidMovePayload => {
            HttpResponse::text(400, "Invalid move payload".to_string())
        }
        BoardError::InvalidNamePayload => {
            HttpResponse::text(400, "Invalid player name".to_string())
        }
    }
}

fn is_card_move_path(path: &str) -> bool {
    path.starts_with("/api/cards/") && path.ends_with("/move")
}

fn is_card_play_path(path: &str) -> bool {
    path.starts_with("/api/cards/") && path.ends_with("/play")
}

fn is_card_take_path(path: &str) -> bool {
    path.starts_with("/api/cards/") && path.ends_with("/take")
}

fn broadcast_views(state: &ApplicationState) {
    let mut subscribers = state
        .subscribers
        .lock()
        .expect("subscribers mutex poisoned");
    let board = state.board.lock().expect("board mutex poisoned");

    subscribers.retain(|subscriber| {
        let view = board.view_for(&subscriber.player_id);
        subscriber.sender.send(view).is_ok()
    });
}

fn handle_event_stream(
    mut stream: TcpStream,
    request: HttpRequest,
    state: Arc<ApplicationState>,
) -> io::Result<()> {
    let session = state.ensure_player(&request);
    let player_id = session.player_id;

    if session.was_created {
        broadcast_views(&state);
    }

    let (sender, receiver) = mpsc::channel::<String>();
    state
        .subscribers
        .lock()
        .expect("subscribers mutex poisoned")
        .push(Subscriber {
            player_id: player_id.clone(),
            sender,
        });

    let initial_view = state.view_for(&player_id);
    write_event_stream_headers(&mut stream, &player_id)?;
    write_sse_board_event(&mut stream, &initial_view)?;

    loop {
        match receiver.recv_timeout(Duration::from_secs(20)) {
            Ok(board_json) => write_sse_board_event(&mut stream, &board_json)?,
            Err(RecvTimeoutError::Timeout) => {
                stream.write_all(b": keep-alive\n\n")?;
                stream.flush()?;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

fn write_event_stream_headers(stream: &mut TcpStream, player_id: &str) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\nSet-Cookie: {}\r\n\r\n",
        player_cookie_header(player_id)
    )
}

fn write_sse_board_event(stream: &mut TcpStream, board_json: &str) -> io::Result<()> {
    write!(stream, "event: board\ndata: {board_json}\n\n")?;
    stream.flush()
}

struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpRequest {
    fn read_from(stream: &mut TcpStream) -> io::Result<Option<Self>> {
        let mut buffer = Vec::new();
        let header_end;

        loop {
            let mut chunk = [0; 1024];
            let bytes_read = stream.read(&mut chunk)?;

            if bytes_read == 0 && buffer.is_empty() {
                return Ok(None);
            }

            buffer.extend_from_slice(&chunk[..bytes_read]);

            if let Some(index) = find_header_end(&buffer) {
                header_end = index;
                break;
            }

            if buffer.len() > 64 * 1024 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HTTP request header is too large",
                ));
            }
        }

        let headers_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
        let content_length = content_length(&headers_text);
        let body_start = header_end + 4;
        let expected_length = body_start + content_length;

        while buffer.len() < expected_length {
            let mut chunk = [0; 1024];
            let bytes_read = stream.read(&mut chunk)?;
            if bytes_read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..bytes_read]);
        }

        let mut header_lines = headers_text.lines();
        let request_line = header_lines.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing HTTP request line")
        })?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default().to_string();
        let raw_path = request_parts.next().unwrap_or("/").to_string();
        let path = raw_path
            .split_once('?')
            .map(|(path, _query)| path)
            .unwrap_or(&raw_path)
            .to_string();
        let headers = parse_headers(header_lines);
        let body = String::from_utf8_lossy(&buffer[body_start..expected_length.min(buffer.len())])
            .to_string();

        Ok(Some(Self {
            method,
            path,
            headers,
            body,
        }))
    }

    fn cookie_value(&self, cookie_name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(name, _value)| name.eq_ignore_ascii_case("cookie"))
            .and_then(|(_name, value)| cookie_value(value, cookie_name))
    }
}

struct HttpResponse {
    status_code: u16,
    content_type: &'static str,
    headers: Vec<String>,
    body: String,
}

impl HttpResponse {
    fn html(body: String) -> Self {
        Self::new(200, "text/html; charset=utf-8", body)
    }

    fn javascript(body: String) -> Self {
        Self::new(200, "application/javascript; charset=utf-8", body)
    }

    fn css(body: String) -> Self {
        Self::new(200, "text/css; charset=utf-8", body)
    }

    fn json(status_code: u16, body: String) -> Self {
        Self::new(status_code, "application/json", body)
    }

    fn text(status_code: u16, body: String) -> Self {
        Self::new(status_code, "text/plain; charset=utf-8", body)
    }

    fn new(status_code: u16, content_type: &'static str, body: String) -> Self {
        Self {
            status_code,
            content_type,
            headers: Vec::new(),
            body,
        }
    }

    fn with_player_cookie(mut self, player_id: &str) -> Self {
        self.headers
            .push(format!("Set-Cookie: {}", player_cookie_header(player_id)));
        self
    }
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        response.status_code,
        reason_phrase(response.status_code),
        response.content_type,
        response.body.as_bytes().len()
    )?;

    for header in response.headers {
        write!(stream, "{header}\r\n")?;
    }

    write!(stream, "\r\n{}", response.body)
}

fn reason_phrase(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        _ => "Internal Server Error",
    }
}

fn parse_headers<'a>(header_lines: impl Iterator<Item = &'a str>) -> Vec<(String, String)> {
    header_lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn cookie_value(cookie_header: &str, cookie_name: &str) -> Option<String> {
    cookie_header.split(';').find_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == cookie_name).then(|| value.to_string())
    })
}

fn player_cookie_header(player_id: &str) -> String {
    format!("{PLAYER_COOKIE_NAME}={player_id}; Path=/; SameSite=Lax")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_card_paths() {
        assert!(is_card_move_path("/api/cards/card-1/move"));
        assert!(is_card_play_path("/api/cards/card-1/play"));
        assert!(is_card_take_path("/api/cards/card-1/take"));
        assert!(!is_card_move_path("/api/cards/card-1"));
    }

    #[test]
    fn parses_content_length_case_insensitively() {
        assert_eq!(content_length("POST / HTTP/1.1\r\ncontent-length: 42"), 42);
    }

    #[test]
    fn parses_player_cookie() {
        assert_eq!(
            cookie_value(
                "theme=dark; six_qui_prend_player=player-2",
                PLAYER_COOKIE_NAME
            ),
            Some("player-2".to_string())
        );
    }
}
