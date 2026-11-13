//! Sending a request: the real network ([`HttpTransport`]), or a fake one in
//! tests. The cookie travels apart from the other headers, so a request can
//! be printed without it, and only ever to HoYoverse's hosts.

use std::fmt;
use std::time::Duration;

use crate::{Error, Result, Secret};

/// The hosts the cookie may go to: HoYoLAB, HoYoverse, and miHoYo's
/// overseas `api-os-takumi`.
const HOYOVERSE_DOMAINS: [&str; 3] = ["hoyolab.com", "hoyoverse.com", "mihoyo.com"];

/// The most a response may weigh: a whole roster's details are well under.
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One request to HoYoLAB.
pub struct Request {
    pub method: Method,
    pub url: String,
    /// The headers but the cookie.
    pub headers: Vec<(&'static str, String)>,
    pub cookie: Secret,
    /// A JSON body, for a POST.
    pub body: Option<String>,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &self.headers)
            .field("cookie", &self.cookie)
            .field("body", &self.body)
            .finish()
    }
}

/// Sends a request and returns the response's body.
pub trait Transport {
    fn send(&self, request: &Request) -> Result<String>;
}

/// Whether `url` is https on one of HoYoverse's hosts.
pub fn is_hoyoverse(url: &str) -> bool {
    let Ok(url) = url::Url::parse(url) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| {
            HOYOVERSE_DOMAINS
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        })
}

/// The real network: https only, no redirect followed (a redirect could
/// carry the cookie elsewhere), 30 seconds at most.
pub struct HttpTransport {
    agent: ureq::Agent,
}

impl HttpTransport {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .https_only(true)
            .max_redirects(0)
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Default for HttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for HttpTransport {
    fn send(&self, request: &Request) -> Result<String> {
        if !is_hoyoverse(&request.url) {
            return Err(Error::NotHoyoverse(request.url.clone()));
        }
        // ureq's errors name the address and the status, never the headers.
        let network = |e: ureq::Error| Error::Network(e.to_string());
        let response = match request.method {
            Method::Get => {
                let mut builder = self.agent.get(&request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(*name, value);
                }
                builder
                    .header("Cookie", request.cookie.expose())
                    .call()
                    .map_err(network)?
            }
            Method::Post => {
                let mut builder = self.agent.post(&request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(*name, value);
                }
                builder
                    .header("Cookie", request.cookie.expose())
                    .header("Content-Type", "application/json")
                    .send(request.body.as_deref().unwrap_or("{}"))
                    .map_err(network)?
            }
        };
        let mut response = response;
        response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cookie_goes_to_hoyoverse_only() {
        for url in [
            "https://sg-public-api.hoyolab.com/event/game_record/genshin/api/character/list",
            "https://api-os-takumi.mihoyo.com/binding/api/getUserGameRolesByCookie",
            "https://api-account-os.hoyoverse.com/binding/api/genAuthKeyByCookieToken",
        ] {
            assert!(is_hoyoverse(url), "{url}");
        }
        for url in [
            "http://sg-public-api.hoyolab.com/",
            "https://hoyolab.com.example.org/",
            "https://evilhoyolab.com/",
            "not a url",
        ] {
            assert!(!is_hoyoverse(url), "{url}");
        }
        let refused = HttpTransport::new().send(&Request {
            method: Method::Get,
            url: "https://example.org/".to_owned(),
            headers: Vec::new(),
            cookie: Secret::new("ltoken_v2=v2_abc"),
            body: None,
        });
        assert!(matches!(refused, Err(Error::NotHoyoverse(_))));
    }

    #[test]
    fn a_request_prints_without_its_cookie() {
        let request = Request {
            method: Method::Post,
            url: "https://sg-public-api.hoyolab.com/".to_owned(),
            headers: vec![("x-rpc-language", "en-us".to_owned())],
            cookie: Secret::new("ltoken_v2=v2_abc"),
            body: Some("{}".to_owned()),
        };
        assert!(!format!("{request:?}").contains("v2_abc"));
    }
}
