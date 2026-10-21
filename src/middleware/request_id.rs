/*
  A short id attached to every request, and to every log line it produces.

  Two problems this solves.

  There was no access log at all: `actix_web::middleware::Logger` was never
  wired, so nothing recorded method, path, status or latency. The only lines in
  the log were the ones handlers chose to write, which makes "which endpoint is
  slow" and "what actually returned that 500" unanswerable.

  And the 12-character reference that `Response::internal_server_error` hands the
  client was minted per error, so it correlated exactly one line. Quoting it told
  you the cause but not the request: no path, no method, no status, no timing.

  The id is generated here, kept in a task-local for the life of the request, and
  used for BOTH - the access-log line and the error reference are now the same
  value. One id in a bug report is enough to reconstruct the whole request.

  A task-local rather than request extensions because `internal_server_error` is
  a free function called from ~100 sites that have no `HttpRequest` in scope.
  Threading a parameter through all of them to carry an id would be a large,
  noisy change; scoping the request future gives the same result at the cost of
  one `scope()` call. Actix polls a request's future on one task, so the value is
  visible throughout the handler, including across `.await` points.
*/

use std::future::Future;

use uuid::Uuid;

tokio::task_local! {
    static REQUEST_ID: String;
}

/// The current request's id, when called from inside a request.
///
/// Returns `None` on a background task - the cron sweep, a `tokio::spawn` that
/// outlives its request - which is honest: those lines belong to no request, and
/// inventing an id would imply a correlation that does not exist.
pub fn current() -> Option<String> {
    REQUEST_ID.try_with(|id| id.clone()).ok()
}

/// A fresh id. Short enough to quote over the phone, random enough not to collide.
///
/// Uses the random tail of a v7 uuid: the leading bits are a timestamp, so two
/// requests in the same millisecond would otherwise share a prefix.
pub fn generate() -> String {
    let uuid = Uuid::now_v7().simple().to_string();
    uuid[20..32].to_string()
}

/// Runs `future` with `id` installed as the current request id.
pub async fn scope<F: Future>(id: String, future: F) -> F::Output {
    REQUEST_ID.scope(id, future).await
}
