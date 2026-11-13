//! The requests: the game accounts behind the cookie, an account's
//! characters and their details, and an authkey for its wish history. Each
//! answer is `{retcode, message, data}`; a `retcode` other than 0 becomes an
//! [`Error`].

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::error::from_retcode;
use crate::transport::{Method, Request, Transport};
use crate::{Cookie, Error, Result, Secret};

/// The game accounts behind a cookie.
const GAME_ROLES_URL: &str =
    "https://api-os-takumi.mihoyo.com/binding/api/getUserGameRolesByCookie";
/// The Battle Chronicle's Genshin endpoints.
const RECORD_URL: &str = "https://sg-public-api.hoyolab.com/event/game_record/genshin/api";
/// Authkeys from the cookie token.
const AUTHKEY_URL: &str =
    "https://api-account-os.hoyoverse.com/binding/api/genAuthKeyByCookieToken";

/// Genshin's overseas game, as HoYoLAB names it.
const GENSHIN_GLOBAL: &str = "hk4e_global";

/// What genshin.py sends: HoYoLAB answers browsers.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36";

/// English, so names match the game data.
const LANGUAGE: &str = "en-us";

/// The overseas server a Genshin UID plays on, from its leading digits:
/// `None` for a UID of the Chinese servers, which HoYoLAB doesn't serve.
pub fn genshin_server(uid: &str) -> Option<&'static str> {
    if uid.len() < 9 || !uid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match &uid[..uid.len() - 8] {
        "6" => Some("os_usa"),
        "7" => Some("os_euro"),
        "8" | "18" => Some("os_asia"),
        "9" => Some("os_cht"),
        _ => None,
    }
}

/// A Genshin Impact account behind the cookie.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GameAccount {
    #[serde(rename = "game_uid")]
    pub uid: String,
    /// `os_euro`, `os_usa`…
    pub region: String,
    pub nickname: String,
    /// Adventure Rank.
    pub level: u32,
    #[serde(default)]
    pub game_biz: String,
}

/// A character an account owns, as the Battle Chronicle lists it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OwnedCharacter {
    /// The game's id, as the game data has it (`10000002` for Ayaka).
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub element: String,
    pub rarity: u8,
    pub level: u8,
    #[serde(rename = "actived_constellation_num")]
    pub constellation: u8,
    /// Its weapon, in the list; the details have it in full.
    #[serde(default)]
    pub weapon: Option<EquippedWeapon>,
}

/// The weapon a character holds.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct EquippedWeapon {
    /// The game's id, as the game data has it (`15502` for Amos' Bow).
    pub id: u32,
    pub name: String,
    pub rarity: u8,
    pub level: u8,
    #[serde(rename = "affix_level")]
    pub refinement: u8,
    /// Its ascension phase, in the details.
    #[serde(default)]
    pub promote_level: Option<u8>,
}

/// One constellation of a character.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Constellation {
    pub id: u32,
    pub pos: u8,
    pub name: String,
    #[serde(default)]
    pub effect: String,
    #[serde(rename = "is_actived")]
    pub active: bool,
}

/// One of a character's skills: its talents (`skill_type` 1) and passives.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Skill {
    #[serde(rename = "skill_id")]
    pub id: u32,
    pub skill_type: u8,
    pub name: String,
    pub level: u8,
}

/// A character in full: the list's entry, its weapon with its ascension,
/// its constellations and its skills.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CharacterDetail {
    pub base: OwnedCharacter,
    pub weapon: EquippedWeapon,
    #[serde(default)]
    pub constellations: Vec<Constellation>,
    #[serde(default)]
    pub skills: Vec<Skill>,
}

/// Talks to HoYoLAB with the player's cookie, which it keeps until dropped.
pub struct Client<'t> {
    cookie: Cookie,
    transport: &'t dyn Transport,
    /// Seconds since the epoch, for the request signature.
    clock: fn() -> u64,
}

impl<'t> Client<'t> {
    pub fn new(cookie: Cookie, transport: &'t dyn Transport) -> Self {
        Self {
            cookie,
            transport,
            clock: now,
        }
    }

    /// The Genshin Impact accounts (overseas) linked to the HoYoLAB account.
    pub fn game_accounts(&self) -> Result<Vec<GameAccount>> {
        #[derive(Deserialize)]
        struct List {
            list: Vec<GameAccount>,
        }
        let data = self.signed(Method::Get, GAME_ROLES_URL, None)?;
        let list: List = parse(data)?;
        Ok(list
            .list
            .into_iter()
            .filter(|account| account.game_biz == GENSHIN_GLOBAL)
            .collect())
    }

    /// The characters `account` owns.
    pub fn characters(&self, account: &GameAccount) -> Result<Vec<OwnedCharacter>> {
        #[derive(Deserialize)]
        struct List {
            list: Vec<OwnedCharacter>,
        }
        let list: List = parse(self.characters_raw(account)?)?;
        Ok(list.list)
    }

    /// The characters' list as HoYoLAB sends it, to look at its fields.
    pub fn characters_raw(&self, account: &GameAccount) -> Result<Value> {
        let body = record_body(account, json!({}))?;
        self.signed(
            Method::Post,
            &format!("{RECORD_URL}/character/list"),
            Some(body),
        )
    }

    /// The characters `ids` of `account`, in full.
    pub fn character_details(
        &self,
        account: &GameAccount,
        ids: &[u32],
    ) -> Result<Vec<CharacterDetail>> {
        #[derive(Deserialize)]
        struct List {
            list: Vec<CharacterDetail>,
        }
        let list: List = parse(self.character_details_raw(account, ids)?)?;
        Ok(list.list)
    }

    /// The details as HoYoLAB sends them, to look at their fields.
    pub fn character_details_raw(&self, account: &GameAccount, ids: &[u32]) -> Result<Value> {
        let body = record_body(account, json!({ "character_ids": ids }))?;
        self.signed(
            Method::Post,
            &format!("{RECORD_URL}/character/detail"),
            Some(body),
        )
    }

    /// An authkey for `account`'s wish history (it lasts a day). Needs
    /// `cookie_token_v2` and `account_id_v2` in the cookie.
    pub fn authkey(&self, account: &GameAccount) -> Result<Secret> {
        #[derive(Deserialize)]
        struct Key {
            authkey: String,
        }
        if !self.cookie.can_make_authkeys() {
            return Err(Error::MissingCookie("cookie_token_v2 and account_id_v2"));
        }
        let body = json!({
            "game_uid": account.uid,
            "region": account.region,
            "game_biz": GENSHIN_GLOBAL,
            "auth_appid": "webview_gacha",
        });
        let data = self.send(Request {
            method: Method::Post,
            url: AUTHKEY_URL.to_owned(),
            headers: vec![("User-Agent", USER_AGENT.to_owned())],
            cookie: self.cookie.header(),
            body: Some(body.to_string()),
        })?;
        let key: Key = parse(data)?;
        // The key comes URL-encoded; it goes into a link encoded again.
        Ok(Secret::new(percent_decode(&key.authkey)))
    }

    /// A request signed as HoYoLAB's web pages sign theirs.
    fn signed(&self, method: Method, url: &str, body: Option<Value>) -> Result<Value> {
        let headers = vec![
            ("User-Agent", USER_AGENT.to_owned()),
            ("x-rpc-app_version", "1.5.0".to_owned()),
            ("x-rpc-client_type", "5".to_owned()),
            ("x-rpc-language", LANGUAGE.to_owned()),
            ("x-rpc-lang", LANGUAGE.to_owned()),
            ("DS", crate::ds::dynamic_secret((self.clock)())),
        ];
        self.send(Request {
            method,
            url: url.to_owned(),
            headers,
            cookie: self.cookie.header(),
            body: body.map(|body| body.to_string()),
        })
    }

    /// Sends `request` and returns the answer's `data`.
    fn send(&self, request: Request) -> Result<Value> {
        #[derive(Deserialize)]
        struct Answer {
            retcode: i64,
            #[serde(default)]
            message: String,
            #[serde(default)]
            data: Value,
        }
        let text = self.transport.send(&request)?;
        let answer: Answer =
            serde_json::from_str(&text).map_err(|e| Error::Format(e.to_string()))?;
        if answer.retcode != 0 {
            return Err(from_retcode(answer.retcode, &answer.message));
        }
        Ok(answer.data)
    }
}

/// A Battle Chronicle request's body: the account, then `extra`'s fields.
fn record_body(account: &GameAccount, extra: Value) -> Result<Value> {
    let role_id: u64 = account
        .uid
        .parse()
        .map_err(|_| Error::Format(format!("{} isn't a UID", account.uid)))?;
    let mut body = json!({ "role_id": role_id, "server": account.region });
    if let (Value::Object(body), Value::Object(extra)) = (&mut body, extra) {
        body.extend(extra);
    }
    Ok(body)
}

fn parse<T: DeserializeOwned>(data: Value) -> Result<T> {
    serde_json::from_value(data).map_err(|e| Error::Format(e.to_string()))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Undoes `%XX` escapes (an authkey has `%2B`, `%2F`, `%3D`).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16);
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((high * 16 + low) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// A request the fake received, its cookie in the clear.
    struct Sent {
        headers: Vec<(&'static str, String)>,
        cookie: String,
        body: Option<String>,
    }

    /// Answers by the URL's end, and keeps what was sent.
    struct FakeApi {
        answers: Vec<(&'static str, String)>,
        sent: RefCell<Vec<Sent>>,
    }

    impl FakeApi {
        fn new(answers: Vec<(&'static str, Value)>) -> Self {
            Self {
                answers: answers
                    .into_iter()
                    .map(|(end, answer)| (end, answer.to_string()))
                    .collect(),
                sent: RefCell::new(Vec::new()),
            }
        }
    }

    impl Transport for FakeApi {
        fn send(&self, request: &Request) -> Result<String> {
            self.sent.borrow_mut().push(Sent {
                headers: request.headers.clone(),
                cookie: request.cookie.expose().to_owned(),
                body: request.body.clone(),
            });
            self.answers
                .iter()
                .find(|(end, _)| request.url.ends_with(end))
                .map(|(_, answer)| answer.clone())
                .ok_or_else(|| Error::Network(format!("no answer for {}", request.url)))
        }
    }

    fn ok(data: Value) -> Value {
        json!({ "retcode": 0, "message": "OK", "data": data })
    }

    fn cookie() -> Cookie {
        Cookie::parse("ltuid_v2=123; ltoken_v2=v2_token; cookie_token_v2=v2_ct; account_id_v2=123")
            .unwrap()
    }

    fn account() -> GameAccount {
        GameAccount {
            uid: "712345678".to_owned(),
            region: "os_euro".to_owned(),
            nickname: "Traveler".to_owned(),
            level: 60,
            game_biz: GENSHIN_GLOBAL.to_owned(),
        }
    }

    #[test]
    fn servers_follow_the_uids_first_digits() {
        assert_eq!(genshin_server("712345678"), Some("os_euro"));
        assert_eq!(genshin_server("612345678"), Some("os_usa"));
        assert_eq!(genshin_server("812345678"), Some("os_asia"));
        assert_eq!(genshin_server("1812345678"), Some("os_asia"));
        assert_eq!(genshin_server("912345678"), Some("os_cht"));
        assert_eq!(genshin_server("112345678"), None, "a Chinese server");
        assert_eq!(genshin_server("71234"), None);
    }

    #[test]
    fn the_genshin_accounts_behind_the_cookie() {
        let api = FakeApi::new(vec![(
            "getUserGameRolesByCookie",
            ok(json!({ "list": [
                { "game_biz": "hkrpg_global", "region": "prod_official_eur",
                  "game_uid": "700000001", "nickname": "Trailblazer", "level": 70 },
                { "game_biz": "hk4e_global", "region": "os_euro", "game_uid": "712345678",
                  "nickname": "Traveler", "level": 60, "is_chosen": true },
            ]})),
        )]);
        let client = Client::new(cookie(), &api);
        assert_eq!(client.game_accounts().unwrap(), [account()]);

        // Signed, in English, with the cookie's HoYoLAB fields only.
        let sent = api.sent.borrow();
        let Sent {
            headers, cookie, ..
        } = &sent[0];
        let header = |name| headers.iter().find(|(n, _)| *n == name).map(|(_, v)| v);
        assert_eq!(header("x-rpc-language").unwrap(), "en-us");
        assert_eq!(header("DS").unwrap().split(',').count(), 3);
        assert_eq!(
            cookie,
            "ltuid_v2=123; ltoken_v2=v2_token; cookie_token_v2=v2_ct; account_id_v2=123"
        );
    }

    #[test]
    fn characters_and_their_details() {
        let api = FakeApi::new(vec![
            (
                "character/list",
                ok(json!({ "list": [{
                    "id": 10000037, "name": "Ganyu", "element": "Cryo", "rarity": 5,
                    "level": 90, "actived_constellation_num": 2, "fetter": 10,
                    "weapon": { "id": 15502, "name": "Amos' Bow", "rarity": 5, "level": 90,
                                "affix_level": 1, "type": 12 }
                }]})),
            ),
            (
                "character/detail",
                ok(json!({ "list": [{
                    "base": { "id": 10000037, "name": "Ganyu", "element": "Cryo", "rarity": 5,
                              "level": 90, "actived_constellation_num": 2 },
                    "weapon": { "id": 15502, "name": "Amos' Bow", "rarity": 5, "level": 90,
                                "affix_level": 1, "promote_level": 6 },
                    "constellations": [{ "id": 371, "pos": 1, "name": "Dew-Drinker",
                                         "effect": "…", "is_actived": true }],
                    "skills": [
                        { "skill_id": 10371, "skill_type": 1, "name": "Liutian Archery", "level": 6 },
                        { "skill_id": 10372, "skill_type": 1, "name": "Trail of the Qilin", "level": 9 },
                        { "skill_id": 10375, "skill_type": 1, "name": "Celestial Shower", "level": 9 },
                        { "skill_id": 371001, "skill_type": 2, "name": "Undivided Heart", "level": 1 }
                    ]
                }]})),
            ),
        ]);
        let client = Client::new(cookie(), &api);
        let list = client.characters(&account()).unwrap();
        assert_eq!(list[0].id, 10000037);
        assert_eq!(list[0].constellation, 2);
        assert_eq!(list[0].weapon.as_ref().unwrap().refinement, 1);

        let details = client.character_details(&account(), &[10000037]).unwrap();
        assert_eq!(details[0].weapon.promote_level, Some(6));
        assert_eq!(details[0].skills.len(), 4);

        // The account goes in the body, the UID as a number.
        let sent = api.sent.borrow();
        let body: Value = serde_json::from_str(sent[1].body.as_deref().unwrap()).unwrap();
        assert_eq!(
            body,
            json!({ "role_id": 712345678u64, "server": "os_euro", "character_ids": [10000037] })
        );
    }

    #[test]
    fn hoyolabs_refusals_become_errors_the_player_can_act_on() {
        for (retcode, check) in [
            (
                10001,
                (|e: &Error| matches!(e, Error::InvalidCookie)) as fn(&Error) -> bool,
            ),
            (10102, |e| matches!(e, Error::DataNotPublic)),
            (10035, |e| matches!(e, Error::Captcha)),
            (10101, |e| matches!(e, Error::TooFrequent)),
            (-1, |e| matches!(e, Error::Api { retcode: -1, .. })),
        ] {
            let api = FakeApi::new(vec![(
                "character/list",
                json!({ "retcode": retcode, "message": "nope", "data": null }),
            )]);
            let error = Client::new(cookie(), &api)
                .characters(&account())
                .unwrap_err();
            assert!(check(&error), "{retcode}: {error}");
        }
    }

    #[test]
    fn an_authkey_needs_the_cookie_token() {
        let api = FakeApi::new(vec![(
            "genAuthKeyByCookieToken",
            ok(json!({ "authkey": "abc%2Bdef%2F%3D%3D", "authkey_ver": 1 })),
        )]);
        let key = Client::new(cookie(), &api).authkey(&account()).unwrap();
        assert_eq!(key.expose(), "abc+def/==");

        let without = Cookie::parse("ltuid_v2=123; ltoken_v2=v2_token").unwrap();
        let error = Client::new(without, &api).authkey(&account()).unwrap_err();
        assert!(matches!(error, Error::MissingCookie(_)));
    }
}
