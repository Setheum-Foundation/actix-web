//! TEMP

use std::{cell::Ref, str};

use actix_http::{http::header, HttpMessage};
use cookie::{Cookie, ParseError as CookieParseError};

struct Cookies(Vec<Cookie<'static>>);

pub trait RequestCookieExt: HttpMessage {
    /// Load request cookies.
    fn cookies(&self) -> Result<Ref<'_, Vec<Cookie<'static>>>, CookieParseError> {
        if self.extensions().get::<Cookies>().is_none() {
            let mut cookies = Vec::new();
            for hdr in self.headers().get_all(header::COOKIE) {
                let s = str::from_utf8(hdr.as_bytes()).map_err(CookieParseError::from)?;
                for cookie_str in s.split(';').map(|s| s.trim()) {
                    if !cookie_str.is_empty() {
                        cookies.push(Cookie::parse_encoded(cookie_str)?.into_owned());
                    }
                }
            }
            self.extensions_mut().insert(Cookies(cookies));
        }

        Ok(Ref::map(self.extensions(), |ext| {
            &ext.get::<Cookies>().unwrap().0
        }))
    }

    /// Return request cookie.
    fn cookie(&self, name: &str) -> Option<Cookie<'static>> {
        if let Ok(cookies) = self.cookies() {
            for cookie in cookies.iter() {
                if cookie.name() == name {
                    return Some(cookie.to_owned());
                }
            }
        }
        None
    }
}

impl RequestCookieExt for crate::HttpRequest {}

pub trait ResponseCookieExt {
    fn cookie<'c>(&mut self, cookie: Cookie<'c>) -> &mut Self;

    fn del_cookie<'a>(&mut self, cookie: &Cookie<'a>) -> &mut Self;
}

impl ResponseCookieExt for crate::dev::HttpResponseBuilder {
    fn cookie<'c>(&mut self, cookie: Cookie<'c>) -> &mut Self {
        // TODO: has different behavior to previous impl since it
        // does not replace cookies of same name

        self.append_header((header::SET_COOKIE, cookie.encoded().to_string()));
        self
    }

    fn del_cookie<'a>(&mut self, cookie: &Cookie<'a>) -> &mut Self {
        let mut removal_cookie = cookie.clone();
        removal_cookie.make_removal();
        self.append_header((header::SET_COOKIE, removal_cookie.encoded().to_string()));
        self
    }
}
