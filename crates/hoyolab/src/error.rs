//! What can go wrong, in words the player can act on. No message ever holds
//! the cookie or an authkey.

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The cookie lacks {0}. Copy the whole Cookie header of a request to hoyolab.com.")]
    MissingCookie(&'static str),
    #[error(
        "HoYoLAB doesn't accept this cookie. Log in to hoyolab.com again, then copy a new one."
    )]
    InvalidCookie,
    #[error(
        "HoYoLAB keeps this data private. On hoyolab.com, open your profile's Battle \
         Chronicle settings and make it public, then try again."
    )]
    DataNotPublic,
    #[error(
        "HoYoLAB asks for a captcha. Open your Battle Chronicle on hoyolab.com once, \
         solve it there, then try again."
    )]
    Captcha,
    #[error("HoYoLAB says the requests come too often. Wait a minute, then try again.")]
    TooFrequent,
    #[error("No Genshin Impact account is linked to this HoYoLAB account.")]
    NoGameAccount,
    #[error("{0} isn't one of HoYoverse's addresses: the cookie is sent to those only.")]
    NotHoyoverse(String),
    #[error("HoYoLAB answered [{retcode}] {message}")]
    Api { retcode: i64, message: String },
    #[error("Couldn't reach HoYoLAB: {0}")]
    Network(String),
    #[error("HoYoLAB's answer doesn't read: {0}")]
    Format(String),
}

/// The error a HoYoLAB `retcode` stands for (genshin.py's `errors.py`).
pub(crate) fn from_retcode(retcode: i64, message: &str) -> Error {
    match retcode {
        -100 | 10001 | 10103 => Error::InvalidCookie,
        10102 => Error::DataNotPublic,
        -110 | 1028 | 10101 | -500004 => Error::TooFrequent,
        10035 | 5003 | 10041 | 1034 => Error::Captcha,
        -10002 => Error::NoGameAccount,
        _ => Error::Api {
            retcode,
            message: message.to_owned(),
        },
    }
}
