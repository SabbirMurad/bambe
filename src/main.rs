use actix_files as StaticResource;
use actix_session::config::{CookieContentSecurity, PersistentSession, TtlExtensionPolicy};
use actix_session::{SessionMiddleware, storage::RedisActorSessionStore};
use actix_web::{
    App, HttpResponse, HttpServer, cookie::Key, cookie::SameSite, cookie::time::Duration, dev,
    dev::Service, error, http, middleware as ActixMiddleware, web,
};
use dotenvy::dotenv as App_env;
use futures_util::future::{self, Either, FutureExt};
use listenfd::ListenFd;
use actix_governor::{Governor, GovernorConfigBuilder};
use std::{env, io};
use tera::{Context, Tera};

mod builtins;
use builtins as BuiltIns;

mod model;
use model as Model;

mod middleware;
use middleware as Middleware;

mod util;
use util as Util;

mod route;
use route as Route;

mod integrations;
use integrations as Integrations;

mod handler;
use handler as Handler;

mod markup;
use markup as Markup;

mod seo;

mod constant;
use constant as Constant;


/// Name of the session cookie. Keep it generic — a cookie name that leaks the
/// product or company is free reconnaissance.
const SESSION_COOKIE_NAME: &str = "sid";

/// The public origin a redirect should point at, built from the host the
/// request arrived with.
///
/// No port, ever. Put this app behind a reverse proxy and it binds a private
/// port the outside world is not supposed to see; a redirect that named that
/// port answered visitors with the public hostname wearing the app's own bind
/// address, e.g. `https://example.com:444/`. `connection_info().host()` is the
/// right source because it prefers a proxy's `X-Forwarded-Host` over the
/// socket, but it still carries whatever port came with it, so the port is
/// dropped here rather than trusted.
///
/// Dropping it resolves to 443, the only port a canonical https URL should
/// name. Serving https on some other port with nothing in front is therefore
/// not supported by this redirect — that is what `APP_HTTP=allow` is for in
/// development, and it short-circuits the whole thing.
fn canonical_origin(host: &str) -> String {
    let host = host.split(':').next().unwrap_or(host);
    format!("https://{}", host)
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    /*
    Loads environment variables from `.env` file to `std::env`
    */
    App_env().ok();

    /*
    Use with log::info!() | log::warn!() | log::error!()
    */
    let _logger = BuiltIns::logger::init();

    /*
    MONGODB CLIENT

    Awaited, and before anything that touches the database. `ClientOptions::parse`
    is async while `MongoDB.connect()` is called from many synchronous sites, so the
    parse happens once here and those sites read the finished client.

    A malformed or missing MONGO_URI stops the process here rather than surfacing
    as a failure on the first request that needs the database.
    */
    BuiltIns::mongo::MongoDB::init()
        .await
        .expect("could not build the MongoDB client (check MONGO_URI)");

    /*
    MONGODB INDEXES AND ONE-OFF MIGRATIONS

    Here rather than in the app factory, for the same reason as the cron below:
    once per process, not once per worker.

    Spawned rather than awaited. `Client::with_options` does not connect eagerly,
    so this is the first thing that actually touches the database - awaiting it
    would make a slow or unreachable MongoDB delay the listener coming up, and
    the failures it reports are not ones the process can act on anyway. Each step
    logs loudly if it could not complete.
    */
    tokio::spawn(async {
        // Unique email indexes on account_core (users) and admin_account (admins).
        Handler::Auth::ensure_indexes().await;

        // Upgrades any password still stored as plaintext by the old scheme.
        // Idempotent, so it costs one scan on a database that is already clean.
        Handler::Auth::migrate_plaintext_passwords().await;

        // Uniqueness and lookup indexes for the storefront collections.
        BuiltIns::indexes::ensure_indexes(&BuiltIns::mongo::MongoDB.connect()).await;

        // Moves products from the old fixed size/colour fields to named
        // options. Idempotent: a converted database costs one empty query.
        Handler::Admin::Product::migrate_size_colour(&BuiltIns::mongo::MongoDB.connect()).await;
    });

    /*
    RUNS CRON JOBS FOR YOUR PROJECT

    Spawned here, before HttpServer::new, so it runs ONCE per process. Putting
    it inside the app factory below would start one copy per worker thread.
    */
    tokio::spawn(async move {
        use tokio::time::{self, Duration};

        let hours: u64 = env::var("CLEANUP_INTERVAL_HOURS")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(24);

        let mut interval = time::interval(Duration::from_secs(hours * 60 * 60));

        loop {
            // The first tick resolves immediately, so a sweep also runs at
            // startup rather than only after the first full interval.
            interval.tick().await;

            /*
            Execute your desired CRON routines here.
            */
            BuiltIns::cron::purge_expired_sessions().await;
        }
    });

    /*
        Sqlite Database Initialization
        Remove the following code block if you are not using this feature.
    */
    log::info!("\nExecuting Sqlite3 Prerequisites...");
    BuiltIns::sqlite::create_initial_tables().expect("Failed to initiate!\n");

    let mut listenfd = ListenFd::from_env();

    let host = env::var("APP_HOST").expect("APP_HOST must be set on .env file");
    let http_port = env::var("APP_HTTP_PORT").expect("APP_HTTP_PORT must be set on .env file");
    let https_port = env::var("APP_HTTPS_PORT").expect("APP_HTTPS_PORT must be set on .env file");

    let redis_host = env::var("REDIS_HOST").expect("REDIS_HOST must be set on .env file");
    let redis_port = env::var("REDIS_PORT").expect("REDIS_PORT must be set on .env file");
    let redis_server = format!("{}:{}", redis_host, redis_port);

    /*
        Use a unique private key for every project.
        Anyone with access to the key can generate authentication cookies
        For any user on the system, which will compromise the authentication system!
    */
    let private_key = env::var("SESSION_KEY").expect("SESSION_KEY must be set on .env file");

    // Per-IP rate limiter (baseline flood/DoS protection for every route). Tunable
    // via env with generous defaults, so normal multi-request page loads pass while
    // a per-IP flood is throttled (429). Built ONCE here and shared across all
    // workers — creating it inside the factory closure below would give each worker
    // its own bucket, multiplying the effective limit by the worker count.
    // NOTE: keys on the peer (connection) IP. Behind a reverse proxy every request
    // would appear from the proxy IP; use a forwarded-header key extractor there.
    let rl_rps: u64 = env::var("RATE_LIMIT_PER_SECOND")
        .ok().and_then(|s| s.parse().ok()).filter(|&n| n > 0).unwrap_or(50);
    let rl_burst: u32 = env::var("RATE_LIMIT_BURST")
        .ok().and_then(|s| s.parse().ok()).filter(|&n| n > 0).unwrap_or(500);
    let governor_conf = GovernorConfigBuilder::default()
        .milliseconds_per_request((1000 / rl_rps).max(1))
        .burst_size(rl_burst)
        .finish()
        .expect("valid rate-limit configuration");

    let http_server = HttpServer::new(move || {
        /*
        IMPORTANT:
        Do not create any global data such as web::Data<T> here
        Multi CPU machine can spawn multiple thread and will create multiple global instance of such data.
        */

        App::new()
            .wrap_fn(|sreq, srv| {
                /*
                  Both redirects, in one layer and in one hop: HTTP -> HTTPS
                  and www -> apex, decided before the handler runs.

                  www -> apex used to run on the response and test
                  request.uri().to_string() for the substring "https://www.",
                  which never matched: a server-side URI is origin-form —
                  "/about" — with no scheme and no host in it at all. It had
                  never once fired, so both hosts answered 200 and a search
                  engine was left to guess which was canonical.

                  `allow` serves plain HTTP as-is, which is what local development
                  needs since the dev certificate is untrusted.
                */
                let allow = env::var("APP_HTTP")
                    .expect("APP_HTTP must be set on .env file")
                    == "allow";

                /*
                  The target used to be host + APP_HTTPS_PORT, this process's
                  own bind port. Served directly that is right; behind a proxy
                  it publishes the port the proxy exists to hide, and every
                  plain-http visitor is sent somewhere they cannot reach. The
                  scheme above comes from X-Forwarded-Proto, so this already
                  fires on what the visitor used rather than on the hop
                  between the proxy and this process. See canonical_origin.
                */
                let host = sreq.connection_info().host().to_owned();
                let on_https = sreq.connection_info().scheme() == "https";
                let on_www = host.starts_with("www.");

                /*
                  One redirect, not two. A visitor arriving on http://www.
                  needs both corrections, and answering with the apex on https
                  straight away spends one hop where chaining them would spend
                  two — which is also one fewer for a crawler to follow.
                */
                if !allow && (!on_https || on_www) {
                    let apex = host.trim_start_matches("www.").to_owned();
                    let url = format!("{}{}", canonical_origin(&apex), sreq.uri());

                    return Either::Right(future::ready(Ok(sreq.into_response(
                        HttpResponse::MovedPermanently()
                            .append_header((http::header::LOCATION, url))
                            .finish(),
                    ))));
                }

                Either::Left(srv.call(sreq).map(|res| res))
            })
            .app_data(web::Data::new(Tera::new("pages/**/*").unwrap()))
            // Render JSON extractor errors in the app's { "message": ... } shape.
            .app_data(web::JsonConfig::default().error_handler(Util::response::json_error_handler))
            .wrap_fn(move |req, srv| {
                /* Custom Error Page Handler */
                srv.call(req).map(|res| {
                    if let Ok(response) = &res {
                        let request = response.request();

                        if response.status() == http::StatusCode::NOT_FOUND
                            && request.method() == http::Method::GET
                        {
                            // 👇 Skip API routes (or any route returning JSON)
                            let path = request.path();
                            let accept_header = request
                                .headers()
                                .get("accept")
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("");

                            /* A page handler that renders its own 404 (an
                               unknown product, say) has already written the
                               body the visitor should see. */
                            let has_page = response
                                .headers()
                                .get(http::header::CONTENT_TYPE)
                                .and_then(|v| v.to_str().ok())
                                .map(|v| v.starts_with("text/html"))
                                .unwrap_or(false);

                            if path.starts_with("/api")
                                || path.starts_with("/image")
                                || accept_header.contains("application/json")
                                || has_page
                            {
                                // Don’t override JSON API 404s
                                return res;
                            }

                            let tera = request.app_data::<web::Data<Tera>>();
                            let template = tera.unwrap();
                            let ctx = Context::new();

                            let res_data = template
                                .render("error/not_found.html", &ctx)
                                .map_err(|e| error::ErrorInternalServerError(e))
                                .unwrap();

                            return Ok(dev::ServiceResponse::new(
                                request.clone(),
                                HttpResponse::NotFound()
                                    .content_type("text/html")
                                    .body(res_data),
                            ));
                        }
                    }
                    res
                })
            })
            .wrap(
                /*
                  Redis session manager based on Cookie
                  Session data gets removed automatically from database when TTL expires.

                  Visit the following link for User Manual
                  https://docs.rs/actix-session/0.7.2/actix_session/struct.Session.html
                */
                SessionMiddleware::builder(
                    RedisActorSessionStore::new(&redis_server),
                    Key::derive_from(&private_key.as_bytes()),
                )
                // Secure everywhere except local development; SameSite=Lax avoids
                // the CSRF exposure of None while still allowing top-level nav.
                // Only the sign-in handlers use the session; the payment
                // gateway's cross-site callback does not, so Lax costs nothing.
                .cookie_secure(
                    std::env::var("APP_STAGE").map(|s| s != "development").unwrap_or(true),
                )
                .cookie_http_only(true)
                .cookie_name(SESSION_COOKIE_NAME.to_owned())
                .cookie_same_site(SameSite::Lax)
                .cookie_content_security(CookieContentSecurity::Signed)
                .session_lifecycle(
                    PersistentSession::default()
                        .session_ttl(Duration::days(15))
                        .session_ttl_extension_policy(TtlExtensionPolicy::OnStateChanges),
                )
                .build(),
            )
            .wrap(BuiltIns::cors::get_policy())
            .wrap(ActixMiddleware::Compress::default())
            /*
              Custom HTTP Headers.

              One DefaultHeaders layer, not one per header — each `.wrap()` is a
              separate middleware in the chain. These are unconditional; anything
              that depends on the request goes in a `wrap_fn` below.

              SAMEORIGIN rather than DENY: still refuses to be framed by anyone
              else, which is what stops clickjacking, but lets the site frame
              its own pages — needed for the responsive preview harness, and
              for anything else that previews a page in place.
            */
            .wrap(
                ActixMiddleware::DefaultHeaders::new()
                    .add(("X-Frame-Options", "SAMEORIGIN"))
                    // Stops a browser from second-guessing Content-Type. Matters
                    // most for the /image routes, which serve client-uploaded
                    // bytes back out.
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("Referrer-Policy", "strict-origin-when-cross-origin"))
                    .add(BuiltIns::csp::get_policy()),
            )
            .wrap_fn(|req, srv| {
                /*
                  Response headers that DefaultHeaders cannot set, in one layer.

                  Both of these need something `DefaultHeaders` has no access to -
                  one needs the request scheme, the other the request path - so
                  neither can be a static header.

                  HSTS: RFC 6797 section 7.2 forbids sending it over non-secure
                  transport, so it is gated on the scheme rather than sent
                  unconditionally. Also suppressed in development, because
                  `includeSubDomains` applies to the HOST and ignores the port -
                  accepting it once from https://localhost pins every localhost
                  service on every port to HTTPS for a year and breaks unrelated
                  local projects.

                  Cache-Control for static assets: scripts and styles are not
                  content-hashed here, so a flat `max-age=86400` meant a fix
                  shipped today reached a returning visitor a day later. Code
                  and configuration therefore revalidate every time — `no-cache`
                  still stores the file, it just has to ask first, and
                  use_last_modified answers almost all of those with a 304.
                  Media keeps the day-long cache: it is large, and a changed
                  picture gets a new id rather than overwriting an old one.
                */
                let secure = req.connection_info().scheme() == "https";
                let production = env::var("APP_STAGE")
                    .map(|stage| stage != "development")
                    .unwrap_or(true);

                srv.call(req).map(move |mut res| {
                    if let Ok(response) = &mut res {
                        if secure && production {
                            response.headers_mut().insert(
                                http::header::STRICT_TRANSPORT_SECURITY,
                                http::header::HeaderValue::from_static(
                                    "max-age=31536000; includeSubDomains",
                                ),
                            );
                        }

                        let path = response.request().path().to_string();
                        if path.contains("/assets/") {
                            let revalidate = path.ends_with(".js")
                                || path.ends_with(".css")
                                || path.ends_with(".json")
                                || path.ends_with(".webmanifest")
                                || path.ends_with(".html");
                            let value = if revalidate {
                                "no-cache"
                            } else {
                                "public, max-age=86400, must-revalidate"
                            };
                            response.headers_mut().insert(
                                http::header::CACHE_CONTROL,
                                http::header::HeaderValue::from_static(value),
                            );
                        }
                    }
                    res
                })
            })
            .service(
                StaticResource::Files::new("/assets/", "assets/")
                    .prefer_utf8(true)
                    .use_last_modified(true),
            )
            // Outermost app middleware: reject per-IP floods with 429 before the
            // session / Redis lookup and handler work run.
            .wrap(Governor::new(&governor_conf))
            /*
              Request id AND access log, in ONE layer, registered last so it is
              the outermost - it times the whole stack and sees the status
              actually sent, including the 429s the rate limiter returns.

              Deliberately one `wrap_fn` rather than `middleware::Logger` plus a
              separate id layer: every `.wrap()` nests another generic into the
              `App` type, and type-checking that expression grows superlinearly.

              The id goes into a task-local for the life of the request, so
              `Response::internal_server_error` reports the SAME id it logs here.
              A client quoting the reference from a 500 leads straight to this
              line - method, path, status, duration.

              `realip_remote_addr` honours X-Forwarded-For, which matters behind a
              proxy where the peer address is always the proxy.
            */
            .wrap_fn(|req, srv| {
                let id = Middleware::RequestId::generate();
                let header = id.clone();
                let started = std::time::Instant::now();
                let method = req.method().clone();
                let path = req.path().to_string();
                let peer = {
                    // Scoped: `connection_info()` borrows `req`, which is moved
                    // into `srv.call` below.
                    let info = req.connection_info();
                    info.realip_remote_addr().unwrap_or("-").to_string()
                };

                let call = srv.call(req);

                Middleware::RequestId::scope(id, async move {
                    let mut res = call.await?;

                    if let Ok(value) = http::header::HeaderValue::from_str(&header) {
                        res.headers_mut().insert(
                            http::header::HeaderName::from_static("x-request-id"),
                            value,
                        );
                    }

                    log::info!(
                        "{} {} {} {} {}ms req={}",
                        peer,
                        method,
                        path,
                        res.status().as_u16(),
                        started.elapsed().as_millis(),
                        header
                    );

                    Ok(res)
                })
            })
            .configure(Route::Auth::router)
            .configure(Route::Account::router)
            .configure(Route::Metadata::router)
            .configure(Route::Image::router)
            .configure(Route::Audio::router)
            .configure(Route::Video::router)
            .configure(Route::Notification::router)
            .configure(Route::Newsletter::router)
            .configure(Route::Admin::router)
            .configure(Route::Category::router)
            .configure(Route::Product::router)
            .configure(Route::Review::router)
            .configure(Route::Cart::router)
            .configure(Route::Wishlist::router)
            .configure(Route::Loyalty::router)
            .configure(Route::RecentlyViewed::router)
            .configure(Route::Address::router)
            .configure(Route::Checkout::router)
            .configure(Route::Order::router)
            .configure(Route::PaymentMethod::router)
            .configure(Route::Payment::router)
            .configure(Route::ReturnRequest::router)
            .configure(Route::SupportTicket::router)
            /* Page routes last: they are the catch-all end of the table, and the
               custom 404 handler renders HTML for them. */
            .configure(Route::Pages::router)
    });

    let http_server = if let Some(listener) = listenfd.take_tcp_listener(0)? {
        http_server.listen(listener)?
    } else {
        http_server.bind((host.as_str(), http_port.parse::<u16>().unwrap()))?
    };

    let http_server = if let Some(listener) = listenfd.take_tcp_listener(1)? {
        http_server.listen_rustls_0_23(listener, BuiltIns::tls::init())?
    } else {
        http_server.bind_rustls_0_23(&format!("{host}:{https_port}"), BuiltIns::tls::init())?
    };

    http_server.run().await
}

#[cfg(test)]
mod tests {
    use super::canonical_origin;

    #[test]
    fn drops_the_apps_own_bind_port() {
        // The bug this function exists for: behind a reverse proxy the
        // redirect named the private port the app binds, not the address
        // visitors actually use.
        assert_eq!(canonical_origin("example.com:444"), "https://example.com");
    }

    #[test]
    fn leaves_a_portless_host_alone() {
        assert_eq!(canonical_origin("example.com"), "https://example.com");
    }

    #[test]
    fn upgrades_the_scheme_not_just_the_port() {
        // The caller passes a bare host; the https is this function's doing.
        assert!(canonical_origin("example.test:80").starts_with("https://"));
    }

    #[test]
    fn handles_a_host_that_is_only_a_port_or_empty() {
        // Nothing should panic on a malformed Host header.
        assert_eq!(canonical_origin(""), "https://");
        assert_eq!(canonical_origin(":444"), "https://");
    }
}
