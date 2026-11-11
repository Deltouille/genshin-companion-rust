//! The game's wish history API: the link that authorises it, and fetching
//! every page of it.
//!
//! The game shows the history in a web page whose address carries a
//! temporary `authkey`. That key also authorises the JSON API behind the page
//! (`getGachaLog`), which returns 20 wishes per page, newest first, for about
//! the last six months.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Deserialize;
use url::Url;

use crate::error::{Error, Result};
use crate::model::GachaType;

const GLOBAL_HOST: &str = "public-operation-hk4e-sg.hoyoverse.com";
const CHINA_HOST: &str = "public-operation-hk4e.mihoyo.com";
const PAGE_SIZE: usize = 20;

/// Gacha types to query. Querying 301 also returns Character Event Wish-2 (400).
pub const QUERIED_TYPES: [GachaType; 5] = [
    GachaType::CharacterEvent,
    GachaType::WeaponEvent,
    GachaType::Standard,
    GachaType::Chronicled,
    GachaType::Beginners,
];

/// A wish history link, reduced to what the API needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GachaLink {
    /// Query parameters of the link that identify and authorise the account.
    params: Vec<(String, String)>,
    china: bool,
}

impl GachaLink {
    /// Finds the link in pasted text (a bare link, or a script's output
    /// around it) and keeps its authorisation.
    pub fn parse(text: &str) -> Result<Self> {
        let candidate = text
            .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`'))
            .filter_map(|token| token.find("https://").map(|at| &token[at..]))
            .find(|token| token.contains("authkey="))
            .ok_or(Error::InvalidLink)?;
        let url = Url::parse(candidate).map_err(|_| Error::InvalidLink)?;

        // Paging and language are chosen per request.
        const PER_REQUEST: [&str; 5] = ["gacha_type", "page", "size", "end_id", "lang"];
        let mut params: Vec<(String, String)> = url
            .query_pairs()
            .into_owned()
            .filter(|(key, _)| !PER_REQUEST.contains(&key.as_str()))
            .collect();
        if !params
            .iter()
            .any(|(key, value)| key == "authkey" && !value.is_empty())
        {
            return Err(Error::InvalidLink);
        }
        for (key, default) in [
            ("authkey_ver", "1"),
            ("sign_type", "2"),
            ("auth_appid", "webview_gacha"),
            ("init_type", "301"),
        ] {
            if !params.iter().any(|(k, _)| k == key) {
                params.push((key.to_owned(), default.to_owned()));
            }
        }

        let china = url
            .host_str()
            .is_some_and(|host| host.ends_with("mihoyo.com"))
            || params
                .iter()
                .any(|(k, v)| k == "game_biz" && v == "hk4e_cn");
        Ok(Self { params, china })
    }

    /// The API address of one page. `end_id` is the id of the last wish of
    /// the previous page ("0" for the first).
    pub fn page_url(&self, gacha_type: GachaType, page: u32, end_id: &str) -> String {
        let host = if self.china { CHINA_HOST } else { GLOBAL_HOST };
        let mut url = Url::parse(&format!("https://{host}/gacha_info/api/getGachaLog"))
            .expect("the API address is valid");
        url.query_pairs_mut()
            .extend_pairs(&self.params)
            .append_pair("lang", "en-us")
            .append_pair("gacha_type", gacha_type.as_str())
            .append_pair("page", &page.to_string())
            .append_pair("size", &PAGE_SIZE.to_string())
            .append_pair("end_id", end_id);
        url.into()
    }
}

/// One wish as the API returns it (every field is text).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ApiRow {
    pub uid: String,
    pub gacha_type: String,
    pub time: String,
    pub name: String,
    pub item_type: String,
    pub rank_type: String,
    pub id: String,
}

/// What a fetch returned: the account and its new wishes, newest first per
/// gacha type.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FetchedHistory {
    pub uid: Option<String>,
    /// The API's region code, such as `os_euro`.
    pub region: Option<String>,
    pub rows: Vec<ApiRow>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FetchProgress {
    pub gacha_type: GachaType,
    pub page: u32,
    /// Wishes received so far, all types together.
    pub fetched: usize,
}

/// Sends a GET request and returns the body.
pub trait Transport {
    fn get(&self, url: &str) -> std::result::Result<String, String>;
}

/// The real network.
pub struct HttpTransport {
    agent: ureq::Agent,
}

impl HttpTransport {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            // The link's key goes in the URL: never over plain http, even
            // after a redirect.
            .https_only(true)
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
    fn get(&self, url: &str) -> std::result::Result<String, String> {
        let mut response = self.agent.get(url).call().map_err(|e| e.to_string())?;
        response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())
    }
}

/// Pacing of requests. The API answers "visit too frequently" when pushed.
#[derive(Clone, Copy, Debug)]
pub struct Pacing {
    /// Pause between two requests.
    pub delay: Duration,
    /// Pause before the first retry; doubled at each further retry.
    pub backoff: Duration,
    pub retries: u32,
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(600),
            backoff: Duration::from_secs(2),
            retries: 4,
        }
    }
}

/// Fetches every gacha type, newest first, up to the first wish that
/// `is_known(uid, id)` reports as already stored.
///
/// Stops with [`Error::Cancelled`] as soon as `cancelled` is set.
pub fn fetch(
    link: &GachaLink,
    transport: &dyn Transport,
    pacing: Pacing,
    is_known: &dyn Fn(&str, &str) -> bool,
    progress: &mut dyn FnMut(FetchProgress),
    cancelled: &AtomicBool,
) -> Result<FetchedHistory> {
    let mut history = FetchedHistory::default();
    for (type_ix, gacha_type) in QUERIED_TYPES.into_iter().enumerate() {
        if type_ix > 0 {
            pause(pacing.delay, cancelled)?;
        }
        let mut page = 1;
        let mut end_id = "0".to_owned();
        loop {
            let (rows, region) = request(
                link, transport, pacing, gacha_type, page, &end_id, cancelled,
            )?;
            if history.region.is_none() {
                history.region = region.filter(|r| !r.is_empty());
            }
            let full_page = rows.len() == PAGE_SIZE;
            let last_id = rows.last().map(|row| row.id.clone());
            let mut reached_known = false;
            for row in rows {
                match &history.uid {
                    Some(uid) if *uid != row.uid => {
                        return Err(Error::MixedAccounts {
                            first: uid.clone(),
                            second: row.uid,
                        });
                    }
                    Some(_) => {}
                    None => history.uid = Some(row.uid.clone()),
                }
                if is_known(&row.uid, &row.id) {
                    reached_known = true;
                    break;
                }
                history.rows.push(row);
            }
            progress(FetchProgress {
                gacha_type,
                page,
                fetched: history.rows.len(),
            });
            let Some(last_id) = last_id.filter(|_| full_page && !reached_known) else {
                break;
            };
            end_id = last_id;
            page += 1;
            pause(pacing.delay, cancelled)?;
        }
    }
    Ok(history)
}

/// One page, retried with growing pauses when the API asks to slow down or
/// the network fails.
fn request(
    link: &GachaLink,
    transport: &dyn Transport,
    pacing: Pacing,
    gacha_type: GachaType,
    page: u32,
    end_id: &str,
    cancelled: &AtomicBool,
) -> Result<(Vec<ApiRow>, Option<String>)> {
    #[derive(Deserialize)]
    struct Envelope {
        retcode: i64,
        #[serde(default)]
        message: String,
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(default)]
        list: Vec<ApiRow>,
        #[serde(default)]
        region: Option<String>,
    }

    let url = link.page_url(gacha_type, page, end_id);
    let mut wait = pacing.backoff;
    let mut attempt = 0;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let retry = match transport.get(&url) {
            Err(network) => Error::Network(network),
            Ok(body) => {
                let envelope: Envelope = serde_json::from_str(&body)
                    .map_err(|e| Error::UnexpectedResponse(e.to_string()))?;
                match envelope.retcode {
                    0 => {
                        let data = envelope
                            .data
                            .ok_or_else(|| Error::UnexpectedResponse("no data".to_owned()))?;
                        return Ok((data.list, data.region));
                    }
                    // authkey timeout
                    -101 => return Err(Error::LinkExpired),
                    // authkey error: a cut or altered link
                    -100 => return Err(Error::LinkRejected),
                    // Visit too frequently.
                    -110 => Error::Api {
                        retcode: envelope.retcode,
                        message: envelope.message,
                    },
                    retcode => {
                        return Err(Error::Api {
                            retcode,
                            message: envelope.message,
                        });
                    }
                }
            }
        };
        if attempt >= pacing.retries {
            return Err(retry);
        }
        attempt += 1;
        pause(wait, cancelled)?;
        wait *= 2;
    }
}

/// Sleeps in short steps so a cancel takes effect quickly.
fn pause(duration: Duration, cancelled: &AtomicBool) -> Result<()> {
    let step = Duration::from_millis(50);
    let mut left = duration;
    while !left.is_zero() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let now = left.min(step);
        std::thread::sleep(now);
        left -= now;
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    const LINK: &str = "https://webstatic-sea.hoyoverse.com/genshin/event/e20190909gacha-v3/index.html\
        ?win_mode=fullscreen&authkey_ver=1&sign_type=2&auth_appid=webview_gacha&init_type=301\
        &gacha_id=abc&lang=fr&region=os_euro&authkey=ab%2Bcd%2F%3D&game_biz=hk4e_global#/log";

    /// Serves canned pages: `(gacha_type, end_id)` → rows.
    #[derive(Default)]
    pub(crate) struct FakeApi {
        pub pages: HashMap<(String, String), Vec<ApiRow>>,
        /// Answers to give before the real ones, in order.
        pub hiccups: RefCell<Vec<std::result::Result<String, String>>>,
        pub requests: RefCell<Vec<String>>,
    }

    impl FakeApi {
        /// `rows` newest first, split into pages of 20 like the API.
        pub fn with_type(mut self, gacha_type: &str, rows: Vec<ApiRow>) -> Self {
            let mut end_id = "0".to_owned();
            for chunk in rows.chunks(PAGE_SIZE) {
                self.pages
                    .insert((gacha_type.to_owned(), end_id.clone()), chunk.to_vec());
                end_id = chunk.last().unwrap().id.clone();
            }
            self
        }
    }

    impl Transport for FakeApi {
        fn get(&self, url: &str) -> std::result::Result<String, String> {
            self.requests.borrow_mut().push(url.to_owned());
            if let Some(answer) = self.hiccups.borrow_mut().pop() {
                return answer;
            }
            let url = Url::parse(url).unwrap();
            let param = |name: &str| {
                url.query_pairs()
                    .find(|(k, _)| k == name)
                    .map(|(_, v)| v.into_owned())
                    .unwrap()
            };
            let rows = self
                .pages
                .get(&(param("gacha_type"), param("end_id")))
                .cloned()
                .unwrap_or_default();
            Ok(serde_json::json!({
                "retcode": 0,
                "message": "OK",
                "data": { "page": "1", "size": "20", "region": "os_euro", "list": rows_json(&rows) }
            })
            .to_string())
        }
    }

    fn rows_json(rows: &[ApiRow]) -> serde_json::Value {
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "uid": r.uid, "gacha_type": r.gacha_type, "item_id": "", "count": "1",
                    "time": r.time, "name": r.name, "lang": "en-us", "item_type": r.item_type,
                    "rank_type": r.rank_type, "id": r.id,
                })
            })
            .collect()
    }

    pub(crate) fn row(id: u64, gacha_type: &str, time: &str, name: &str, rank: &str) -> ApiRow {
        let item_type = if ["Slingshot", "Debate Club", "Cool Steel"].contains(&name) {
            "Weapon"
        } else {
            "Character"
        };
        ApiRow {
            uid: "700000001".to_owned(),
            gacha_type: gacha_type.to_owned(),
            time: time.to_owned(),
            name: name.to_owned(),
            item_type: item_type.to_owned(),
            rank_type: rank.to_owned(),
            id: id.to_string(),
        }
    }

    pub(crate) const NO_WAIT: Pacing = Pacing {
        delay: Duration::ZERO,
        backoff: Duration::ZERO,
        retries: 3,
    };

    fn run(api: &FakeApi, is_known: &dyn Fn(&str, &str) -> bool) -> Result<FetchedHistory> {
        let link = GachaLink::parse(LINK).unwrap();
        fetch(
            &link,
            api,
            NO_WAIT,
            is_known,
            &mut |_| {},
            &AtomicBool::new(false),
        )
    }

    #[test]
    fn links_are_found_in_pasted_text_and_keep_their_key() {
        let pasted = format!("Link copied!\n\"{LINK}\"\n");
        let link = GachaLink::parse(&pasted).unwrap();
        let url = Url::parse(&link.page_url(GachaType::CharacterEvent, 2, "123")).unwrap();
        assert_eq!(url.host_str(), Some(GLOBAL_HOST));
        let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["authkey"], "ab+cd/=");
        assert_eq!(params["lang"], "en-us");
        assert_eq!(params["gacha_type"], "301");
        assert_eq!(params["end_id"], "123");
        assert_eq!(params["region"], "os_euro");
        assert_eq!(url.query_pairs().filter(|(k, _)| k == "lang").count(), 1);

        assert!(matches!(
            GachaLink::parse("https://example.com/?a=1"),
            Err(Error::InvalidLink)
        ));
        assert!(matches!(GachaLink::parse("hello"), Err(Error::InvalidLink)));
    }

    #[test]
    fn china_links_use_the_china_server() {
        let link = GachaLink::parse(
            "https://webstatic.mihoyo.com/hk4e/event/e20190909gacha-v3/index.html?authkey=x&game_biz=hk4e_cn",
        )
        .unwrap();
        assert!(
            link.page_url(GachaType::Standard, 1, "0")
                .contains(CHINA_HOST)
        );
    }

    #[test]
    fn pages_until_the_end_of_the_history() {
        let character: Vec<ApiRow> = (0..45)
            .map(|i| row(1000 - i, "301", "2024-01-10 12:00:00", "Slingshot", "3"))
            .collect();
        let api = FakeApi::default().with_type("301", character);
        let history = run(&api, &|_, _| false).unwrap();

        assert_eq!(history.rows.len(), 45);
        assert_eq!(history.uid.as_deref(), Some("700000001"));
        assert_eq!(history.region.as_deref(), Some("os_euro"));
        // 3 pages of character wishes, then one empty page for each other type.
        assert_eq!(api.requests.borrow().len(), 3 + 4);
    }

    #[test]
    fn stops_at_the_first_wish_already_stored() {
        let character: Vec<ApiRow> = (0..45)
            .map(|i| row(1000 - i, "301", "2024-01-10 12:00:00", "Slingshot", "3"))
            .collect();
        let api = FakeApi::default().with_type("301", character);
        let history = run(&api, &|_, id| id == "990").unwrap();

        assert_eq!(history.rows.len(), 10);
        assert_eq!(api.requests.borrow().len(), 1 + 4);
    }

    #[test]
    fn slows_down_when_asked_and_reports_expired_links() {
        let api = FakeApi::default();
        api.hiccups.borrow_mut().extend([
            Ok(r#"{"retcode":-110,"message":"visit too frequently","data":null}"#.to_owned()),
            Err("connection reset".to_owned()),
        ]);
        assert!(run(&api, &|_, _| false).is_ok());

        let api = FakeApi::default();
        api.hiccups.borrow_mut().push(Ok(
            r#"{"retcode":-101,"message":"authkey timeout","data":null}"#.to_owned(),
        ));
        assert!(matches!(run(&api, &|_, _| false), Err(Error::LinkExpired)));
    }

    #[test]
    fn two_accounts_in_one_history_are_refused() {
        let mut other = row(998, "301", "2024-01-10 12:00:00", "Slingshot", "3");
        other.uid = "800000002".to_owned();
        let api = FakeApi::default().with_type(
            "301",
            vec![
                row(999, "301", "2024-01-10 12:00:00", "Slingshot", "3"),
                other,
            ],
        );
        assert!(matches!(
            run(&api, &|_, _| false),
            Err(Error::MixedAccounts { .. })
        ));
    }

    #[test]
    fn cancelling_stops_the_fetch() {
        let api = FakeApi::default();
        let link = GachaLink::parse(LINK).unwrap();
        let cancelled = AtomicBool::new(true);
        let result = fetch(&link, &api, NO_WAIT, &|_, _| false, &mut |_| {}, &cancelled);
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(api.requests.borrow().is_empty());
    }
}
