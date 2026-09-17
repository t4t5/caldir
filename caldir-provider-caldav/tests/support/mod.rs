use http_body_util::{BodyExt, Full};
use hyper::{
    Response,
    body::{Bytes, Incoming},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use std::sync::{Arc, Mutex};
use tokio::{net::TcpListener, task::JoinHandle};

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
    pub report_status: Option<u16>,
    pub reject_uid_filter: bool,
    pub write_status: Option<u16>,
    pub concurrent_data: Option<String>,
    pub omit_etag: bool,
    pub fail_readback: bool,
    pub bump_sibling_timestamps: bool,
}
impl State {
    fn respond(&mut self, request: Request) -> Response<Full<Bytes>> {
        let mut response = Response::builder();
        let (status, body) = match request.method.as_str() {
            "GET" if self.read_status.is_some() => (self.read_status.unwrap(), String::new()),
            "GET" if self.fail_readback && self.version > 1 => (503, String::new()),
            "GET" if request.path == self.href && self.data.is_some() => {
                if !self.omit_etag {
                    response = response.header("ETag", format!("\"{}\"", self.version));
                }
                (200, self.data.clone().unwrap())
            }
            "GET" => (404, String::new()),
            "REPORT" if self.read_status.is_some() => (self.read_status.unwrap(), String::new()),
            "REPORT" if self.reject_uid_filter && request.body.contains("<C:prop-filter") => {
                (412, String::new())
            }
            "REPORT" if self.report_status.is_some() => {
                (self.report_status.unwrap(), String::new())
            }
            "REPORT" => {
                let response = self.data.as_ref().map(|data| format!(r#"<response><href>{}</href><propstat><prop><getetag>"{}"</getetag><C:calendar-data>{}</C:calendar-data></prop><status>HTTP/1.1 200 OK</status></propstat></response>"#, self.href, self.version, data.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"))).unwrap_or_default();
                let collection = if !request.body.contains("<C:prop-filter")
                    && !request.body.contains("<C:time-range")
                {
                    r#"<response><href>/calendar/</href><propstat><prop><getetag>"collection"</getetag></prop><status>HTTP/1.1 200 OK</status></propstat><propstat><prop><C:calendar-data/></prop><status>HTTP/1.1 404 Not Found</status></propstat></response>"#
                } else {
                    ""
                };
                (
                    207,
                    format!(
                        r#"<multistatus xmlns="DAV:" xmlns:C="urn:ietf:params:xml:ns:caldav">{collection}{response}</multistatus>"#
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
        response
            .status(status)
            .body(Full::new(body.into()))
            .unwrap()
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
            report_status: None,
            reject_uid_filter: false,
            write_status: None,
            concurrent_data: None,
            omit_etag: false,
            fail_readback: false,
            bump_sibling_timestamps: false,
        }));
        let shared = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let service = service_fn(|request: hyper::Request<Incoming>| async {
                    let (parts, body) = request.into_parts();
                    let request = Request {
                        method: parts.method.to_string(),
                        path: parts.uri.path().to_owned(),
                        headers: parts
                            .headers
                            .iter()
                            .map(|(n, v)| (n.to_string(), v.to_str().unwrap().to_owned()))
                            .collect(),
                        body: String::from_utf8(body.collect().await?.to_bytes().to_vec()).unwrap(),
                    };
                    Ok::<_, hyper::Error>(shared.lock().unwrap().respond(request))
                });
                http1::Builder::new()
                    .keep_alive(false)
                    .serve_connection(TokioIo::new(socket), service)
                    .await
                    .unwrap();
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
