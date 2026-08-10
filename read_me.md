<p align="center">
  <img src="assets/brand/bambe-banner.png" alt="Bambe — style that defines you" width="880">
</p>

If you already know about the project run the code below to run it

```bash
systemfd --no-pid -s http::0.0.0.0:8080 -- cargo watch -x run
```

OR

```bash
systemfd --no-pid -s http::0.0.0.0:8080 -- \
  cargo watch -i ".cargo/*" -i "target/*" -i ".git/*" -i "static/*" -i "logs/*" -x run
```

> **Bind to `0.0.0.0`, not the default loopback.** Plain `-s http::8080` binds the
> HTTP socket to `127.0.0.1`, so a phone/emulator on the LAN cannot reach
> `http://<your-ip>:8080` (only `localhost` can). Using `http::0.0.0.0:8080`
> exposes it on all interfaces for on-device testing. HTTPS (8443) is bound by
> the app itself to `APP_HOST` (0.0.0.0), so it's already LAN-reachable — but note
> rustls rejects a TLS handshake made to a raw **IP** address (SNI must be a
> hostname), so for IP-based dev use the **plain HTTP** URL, not HTTPS. Also open
> inbound TCP 8080 in the OS firewall. Alternatively, plain `cargo run` (no
> systemfd) binds HTTP to `APP_HOST:8080` directly.

If you don't know about the project run the code below to read the full documentation

```bash
mkdocs serve
```
