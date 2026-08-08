/*
  HTTP Header - Content Security Policy
  Read - https://infosec.mozilla.org/guidelines/web_security#content-security-policy for more details
*/

pub fn get_policy() -> (String, String) {
    let policy = format!("");

    ("Content-Security-Policy".to_owned(), policy)
}
