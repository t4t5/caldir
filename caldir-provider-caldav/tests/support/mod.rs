use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

pub fn component(rid: Option<&str>) -> String {
    let recurrence = match rid {
        Some(rid) => format!("RECURRENCE-ID:{rid}\r\n"),
        None => "RRULE:FREQ=WEEKLY\r\n".into(),
    };
    format!(
        "BEGIN:VEVENT\r\nUID:series\r\nDTSTART:20260921T070000Z\r\nLAST-MODIFIED:20260901T000000Z\r\nSUMMARY:Water plant\r\n{recurrence}END:VEVENT\r\n"
    )
}

pub fn calendar(components: &[String]) -> String {
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{}END:VCALENDAR\r\n",
        components.join("")
    )
}

pub fn event(rid: Option<&str>) -> caldir_core::Event {
    caldir_core::Event::from_ics_str(&calendar(&[component(rid)]))
        .unwrap()
        .remove(0)
        .unwrap()
}

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}
impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

pub struct State {
    pub data: Option<String>,
    pub href: String,
    pub version: usize,
    pub requests: Vec<Request>,
    pub read_status: Option<u16>,
    pub write_status: Option<u16>,
    pub concurrent_data: Option<String>,
    pub omit_etag: bool,
    pub fail_readback: bool,
    pub bump_sibling_timestamps: bool,
}
impl State {
    fn respond(&mut self, request: Request) -> (u16, String, String) {
        let mut etag = String::new();
        let (status, body) = match request.method.as_str() {
            "GET" if self.read_status.is_some() => (self.read_status.unwrap(), String::new()),
            "GET" if self.fail_readback && self.version > 1 => (503, String::new()),
            "GET" if request.path == self.href && self.data.is_some() => {
                if !self.omit_etag {
                    etag = format!("ETag: \"{}\"\r\n", self.version);
                }
                (200, self.data.clone().unwrap())
            }
            "GET" => (404, String::new()),
            "REPORT" if self.read_status.is_some() => (self.read_status.unwrap(), String::new()),
            "REPORT" => {
                let response = self.data.as_ref().map(|data| format!(r#"<response><href>{}</href><propstat><prop><getetag>"{}"</getetag><C:calendar-data>{}</C:calendar-data></prop><status>HTTP/1.1 200 OK</status></propstat></response>"#, self.href, self.version, data.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"))).unwrap_or_default();
                (
                    207,
                    format!(
                        r#"<multistatus xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">{response}</multistatus>"#
                    ),
                )
            }
            "PUT" | "DELETE" => {
                if let Some(data) = self.concurrent_data.take() {
                    self.data = Some(data);
                    self.version += 1;
                }
                let matches = if self.data.is_some() {
                    request.header("if-match") == Some(format!("\"{}\"", self.version).as_str())
                } else {
                    request.header("if-none-match") == Some("*")
                };
                if let Some(status) = self.write_status {
                    (status, String::new())
                } else if !matches {
                    (412, String::new())
                } else {
                    self.version += 1;
                    self.href = request.path.clone();
                    self.data = if request.method == "DELETE" {
                        None
                    } else {
                        let mut data = request.body.clone();
                        if self.bump_sibling_timestamps {
                            data = data
                                .lines()
                                .map(|line| {
                                    if line.starts_with("LAST-MODIFIED:") {
                                        format!("LAST-MODIFIED:20260917T0000{:02}Z", self.version)
                                    } else {
                                        line.to_owned()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\r\n")
                                + "\r\n";
                        }
                        Some(data)
                    };
                    (204, String::new())
                }
            }
            _ => (500, String::new()),
        };
        self.requests.push(request);
        (status, body, etag)
    }
}

pub struct Server {
    pub url: String,
    pub state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}
impl Server {
    pub async fn new(data: Option<String>, assigned_href: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/calendar/", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State {
            data,
            href: if assigned_href {
                "/calendar/server-assigned.ics"
            } else {
                "/calendar/series.ics"
            }
            .into(),
            version: 1,
            requests: Vec::new(),
            read_status: None,
            write_status: None,
            concurrent_data: None,
            omit_etag: false,
            fail_readback: false,
            bump_sibling_timestamps: false,
        }));
        let shared = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let header_end = loop {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let header = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let mut lines = header.lines();
                let mut first = lines.next().unwrap().split_whitespace();
                let method = first.next().unwrap().to_owned();
                let path = first.next().unwrap().to_owned();
                let headers: Vec<_> = lines
                    .filter_map(|line| line.split_once(':'))
                    .map(|(n, v)| (n.to_lowercase(), v.trim().to_owned()))
                    .collect();
                let length: usize = headers
                    .iter()
                    .find(|(n, _)| n == "content-length")
                    .map(|(_, v)| v.parse().unwrap())
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                }
                let request = Request {
                    method,
                    path,
                    headers,
                    body: String::from_utf8(bytes[header_end..header_end + length].to_vec())
                        .unwrap(),
                };
                let (status, body, etag) = shared.lock().unwrap().respond(request);
                let response = format!(
                    "HTTP/1.1 {status} Mock\r\nConnection: close\r\nContent-Length: {}\r\n{etag}\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        Self { url, state, task }
    }
    pub fn events(&self) -> Vec<caldir_core::Event> {
        self.state
            .lock()
            .unwrap()
            .data
            .as_ref()
            .map(|data| {
                caldir_core::Event::from_ics_str(data)
                    .unwrap()
                    .into_iter()
                    .map(Result::unwrap)
                    .collect()
            })
            .unwrap_or_default()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
