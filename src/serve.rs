use std::error::Error;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::Command as ProcessCommand;

use fence::{DecisionRecordOptions, FenceManager};
use serde::{Deserialize, Serialize};

pub(crate) fn run_serve(host: IpAddr, port: u16, open_browser: bool) -> Result<(), Box<dyn Error>> {
    let bind_host = if host.is_unspecified() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host
    };
    let listener = TcpListener::bind(SocketAddr::new(bind_host, port))?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}");

    println!("Fence UI running at {url}");
    println!("Press Ctrl+C to stop.");
    if open_browser {
        open_url(&url);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_http_request(stream) {
                    eprintln!("Request failed: {err}");
                }
            }
            Err(err) => eprintln!("Connection failed: {err}"),
        }
    }

    Ok(())
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let command = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "start", url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let command = ("xdg-open", vec![url]);

    let _ = ProcessCommand::new(command.0).args(command.1).spawn();
}

#[derive(Debug, Deserialize)]
struct WebEditDecision {
    title: Option<String>,
    optional_tags: Option<Vec<String>>,
    owner: Option<String>,
    reviewer: Option<String>,
    rationale: Option<String>,
    consequences: Option<String>,
    review_due: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebReviewDecision {
    review_due: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebReplaceDecision {
    message: String,
    category: Option<String>,
    title: Option<String>,
    optional_tags: Option<Vec<String>>,
    owner: Option<String>,
    reviewer: Option<String>,
    rationale: Option<String>,
    consequences: Option<String>,
    review_due: Option<String>,
    links: Option<Vec<String>>,
}

fn handle_http_request(mut stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let method = request_line
        .split_whitespace()
        .next()
        .unwrap_or("GET")
        .to_string();
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let trimmed = header.trim_end();
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed
            .strip_prefix("Content-Length:")
            .or_else(|| trimmed.strip_prefix("content-length:"))
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }

    let mut body_bytes = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body_bytes)?;
    }
    let request_body = String::from_utf8_lossy(&body_bytes).to_string();

    let (status, content_type, body) = match route_http_request(&method, &path, &request_body) {
        Ok(response) => response,
        Err(err) => (
            "HTTP/1.1 500 Internal Server Error",
            "text/plain; charset=utf-8",
            format!("Request failed: {err}"),
        ),
    };

    let response = format!(
        "{status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn route_http_request(
    method: &str,
    path: &str,
    request_body: &str,
) -> Result<(&'static str, &'static str, String), Box<dyn Error>> {
    if method == "GET" {
        return Ok(match path {
            "/" | "/index.html" => (
                "HTTP/1.1 200 OK",
                "text/html; charset=utf-8",
                fence::render_site_html()?,
            ),
            "/api/decisions" => (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                serde_json::to_string(&fence::read_log_entries()?)?,
            ),
            "/api/stats" => (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                serde_json::to_string(&fence::decision_status_counts()?)?,
            ),
            "/health" => (
                "HTTP/1.1 200 OK",
                "application/json; charset=utf-8",
                "{\"status\":\"ok\"}".to_string(),
            ),
            _ => (
                "HTTP/1.1 404 Not Found",
                "text/plain; charset=utf-8",
                "Not found".to_string(),
            ),
        });
    }

    if method != "POST" {
        return Ok((
            "HTTP/1.1 405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "Method not allowed".to_string(),
        ));
    }

    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() != 4 || segments[0] != "api" || segments[1] != "decisions" {
        return Ok((
            "HTTP/1.1 404 Not Found",
            "text/plain; charset=utf-8",
            "Not found".to_string(),
        ));
    }

    let id = segments[2];
    let action = segments[3];
    match action {
        "deprecate" => {
            if fence::deprecate_decision(id)? {
                Ok(json_http(
                    "HTTP/1.1 200 OK",
                    &serde_json::json!({ "ok": true }),
                )?)
            } else {
                Ok((
                    "HTTP/1.1 404 Not Found",
                    "text/plain; charset=utf-8",
                    "Decision not found".to_string(),
                ))
            }
        }
        "approve" => {
            if let Some(decision) = fence::approve_decision(id)? {
                Ok(json_http("HTTP/1.1 200 OK", &decision)?)
            } else {
                Ok((
                    "HTTP/1.1 404 Not Found",
                    "text/plain; charset=utf-8",
                    "Decision not found".to_string(),
                ))
            }
        }
        "review" => {
            let payload: WebReviewDecision = serde_json::from_str(request_body)?;
            if let Some(decision) = fence::review_decision(id, payload.review_due.as_deref())? {
                Ok(json_http("HTTP/1.1 200 OK", &decision)?)
            } else {
                Ok((
                    "HTTP/1.1 404 Not Found",
                    "text/plain; charset=utf-8",
                    "Decision not found".to_string(),
                ))
            }
        }
        "edit" => {
            let payload: WebEditDecision = serde_json::from_str(request_body)?;
            if let Some(decision) = fence::update_decision(id, |decision| {
                if let Some(title) = payload.title {
                    decision.title = optional_value(title);
                }
                if let Some(tags) = payload.optional_tags {
                    decision.optional_tags = tags;
                }
                if let Some(owner) = payload.owner {
                    decision.owner = optional_value(owner);
                }
                if let Some(reviewer) = payload.reviewer {
                    decision.reviewer = optional_value(reviewer);
                }
                if let Some(rationale) = payload.rationale {
                    decision.rationale = optional_value(rationale);
                }
                if let Some(consequences) = payload.consequences {
                    decision.consequences = optional_value(consequences);
                }
                if let Some(review_due) = payload.review_due {
                    decision.review_due = fence::normalize_review_due(Some(&review_due))?;
                }
                Ok(())
            })? {
                Ok(json_http("HTTP/1.1 200 OK", &decision)?)
            } else {
                Ok((
                    "HTTP/1.1 404 Not Found",
                    "text/plain; charset=utf-8",
                    "Decision not found".to_string(),
                ))
            }
        }
        "replace" => {
            let payload: WebReplaceDecision = serde_json::from_str(request_body)?;
            let decision = FenceManager::record_with_details(
                &payload.message,
                DecisionRecordOptions {
                    category: parse_category(payload.category),
                    optional_tags: payload.optional_tags.unwrap_or_default(),
                    replaces: Some(id.to_string()),
                    review_due: payload.review_due,
                    title: payload.title,
                    rationale: payload.rationale,
                    consequences: payload.consequences,
                    links: payload.links.unwrap_or_default(),
                    owner: payload.owner,
                    reviewer: payload.reviewer,
                },
            )?;
            Ok(json_http("HTTP/1.1 201 Created", &decision)?)
        }
        _ => Ok((
            "HTTP/1.1 404 Not Found",
            "text/plain; charset=utf-8",
            "Not found".to_string(),
        )),
    }
}

fn json_http<T: Serialize>(
    status: &'static str,
    value: &T,
) -> Result<(&'static str, &'static str, String), serde_json::Error> {
    Ok((
        status,
        "application/json; charset=utf-8",
        serde_json::to_string(value)?,
    ))
}
fn optional_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_category(value: Option<String>) -> fence::DecisionCategory {
    let normalized = value.unwrap_or_else(|| "gen".to_string()).to_lowercase();

    match normalized.as_str() {
        "arch" | "architecture" => fence::DecisionCategory::Architecture,
        "tech" | "technical" => fence::DecisionCategory::Technical,
        "prod" | "product" => fence::DecisionCategory::Product,
        "sec" | "security" => fence::DecisionCategory::Security,
        "gen" | "general" => fence::DecisionCategory::General,
        _ => fence::DecisionCategory::General,
    }
}
