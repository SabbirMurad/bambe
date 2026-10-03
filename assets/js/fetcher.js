import { toast } from './toast.js';
import { session } from './session.js';

class Fetcher {
    /**
     * Serialises a query object.
     *
     * The old inline version appended every key once unconditionally and
     * then again inside its else-branch, so `{rating: 5}` went out as
     * `rating=5&rating=5`. actix's `web::Query` deserialises a non-Vec
     * field from a repeated key as an error, so every single-value filter
     * came back 400. It also never encoded anything, so a search term
     * containing `&` or `#` truncated the rest of the query.
     *
     * Arrays still expand to a repeated key, which is what the endpoints
     * taking a list expect.
     *
     * @param {Object} query
     * @returns {string} an encoded query string, without the leading "?"
     */
    static buildQueryString(query = {}) {
        const params = new URLSearchParams();

        for (const key in query) {
            const value = query[key];
            if (value === undefined || value === null || value === '') continue;

            if (Array.isArray(value)) {
                value.forEach((item) => params.append(key, item));
            } else {
                params.append(key, value);
            }
        }

        return params.toString();
    }

    /**
     *
     * @param {string} options.endpoint - The endpoint to send the request to.
     * @param {Object} options.headers - The headers to send the request with.
     * @param {string|Object} options.body - The body to send the request with.
     * @returns {Promise<FetchResult>}
     */
    static async post({
        endpoint,
        headers = {},
        body,
        showError = true,
    }) {
        return await this.#commonMethods({
            endpoint: endpoint,
            method: "POST",
            headers: headers,
            body: body,
            showError: showError
        })
    }

    /**
     * 
     * @param {string} options.endpoint - The endpoint to send the request to.
     * @param {Object} options.headers - The headers to send the request with.
     * @param {string|Object} options.body - The body to send the request with.
     * @returns {Promise<FetchResult>}
     */
    static async put({
        endpoint,
        headers = {},
        body,
        showError = true,
    }) {
        return await this.#commonMethods({
            endpoint: endpoint,
            method: "PUT",
            headers: headers,
            body: body,
            showError: showError
        })
    }

    /**
     * 
     * @param {string} options.endpoint - The endpoint to send the request to.
     * @param {Object} options.headers - The headers to send the request with.
     * @param {string|Object} options.body - The body to send the request with.
     * @returns {Promise<FetchResult>}
     */
    static async patch({
        endpoint,
        headers = {},
        body,
        showError = true,
    }) {
        return await this.#commonMethods({
            endpoint: endpoint,
            method: "PATCH",
            headers: headers,
            body: body,
            showError: showError
        })
    }

    /**
     * 
     * @param {string} options.endpoint - The endpoint to send the request to.
     * @param {Object} options.headers - The headers to send the request with.
     * @param {Object} options.query - The query parameters to send the request with.
     * @returns {Promise<FetchResult>}
     */
    static async delete({
        endpoint,
        headers = {},
        query = {},
        showError = true,
    }) {
        const queryStr = Fetcher.buildQueryString(query);

        return await this.#commonMethods({
            endpoint: endpoint,
            method: "DELETE",
            headers: headers,
            query: queryStr,
            showError: showError
        })
    }

    /**
     * 
     * @param {string} options.endpoint - The endpoint to send the request to.
     * @param {Object} options.headers - The headers to send the request with.
     * @param {Object} options.query - The query parameters to send the request with.
     * @returns {Promise<FetchResult>}
     */
    static async get({
        endpoint,
        headers = {},
        query = {},
        showError = true,
    }) {
        const queryStr = Fetcher.buildQueryString(query);

        return await this.#commonMethods({
            endpoint: endpoint,
            method: "GET",
            headers: headers,
            query: queryStr,
            showError: showError
        })
    }

    /**
     * 
     * @param {string} options.endpoint - The endpoint to send the request to.
     * @param {string} options.method - The method to send the request with.
     * @param {Object} options.headers - The headers to send the request with.
     * @param {string|Object} options.body - The body to send the request with.
     * @param {Object} options.query - The query parameters to send the request with.
     * @returns {Promise<FetchResult>}
     */
    static async #commonMethods({
        endpoint,
        method,
        headers = {},
        body,
        query = '',
        showError = true,
        // Set only by the 401 path below, so one request can be retried
        // after a refresh but a second 401 falls through as a failure.
        isRetry = false,
    }) {
        let _headers = {
            "Accept": "application/json",
            "Content-Type": "application/json",
            ...headers,
        };

        /*
          Refresh before sending, not only after a 401.

          The cart, the quote and the product page read the caller through
          `optional_access`, which treats a token it cannot verify as no
          token at all — so an expired one comes back 200 with somebody
          else's answer: an empty guest cart. Nothing 401s, nothing
          retries, and the next add-to-cart mints a fresh anonymous cart
          while the real one sits there. Waiting for a 401 only works on
          the endpoints that insist on a token; this covers the rest.
        */
        if (session.token() && session.isExpired() && !isRetry) {
            await session.refresh();
        }

        // Every protected handler reads `Authorization: Bearer <token>`
        // (Middleware::Auth::require_access). Stamped here so no caller has
        // to remember it. An explicit header in `headers` still wins.
        const token = session.token();
        if (token && !_headers["Authorization"]) {
            _headers["Authorization"] = "Bearer " + token;
        }

        /*
          The cart is the one surface that works signed out, keyed on a
          client-minted id (`resolve_owner`, src/handler/cart.rs). Only the
          cart routes get it, and only while signed out — so a visitor who
          never opens the cart is never given an id at all.
        */
        if (!token && endpoint.startsWith("/v1/cart")) {
            _headers["X-Guest-Session"] = session.guestId();
        }

        let url = (window.projectDomain || '') + "/api" + endpoint;
        if (query) {
            url += "?" + query;
        }

        let reqObject = {
            method: method,
            headers: _headers,
        };

        if (body) {
            reqObject.body = JSON.stringify(body);
        }

        let response;
        try {
            response = await fetch(url, reqObject);
        } catch (error) {
            if (showError) {
                toast.setNotification({
                    type: 'error',
                    message: error.toString(),
                })
            }

            return new FetchResult({
                ok: false,
                status: -1,
                data: null,
                error: error.toString(),
            })
        }

        /*
          An expired access token is the ordinary case, not an error: swap
          it for a fresh one and replay the request once. A second 401
          means the refresh token is gone too, and `session.refresh()` has
          already cleared the session — that falls through to the normal
          error path so the page can decide whether to bounce to login.
        */
        if (response.status === 401 && !isRetry && token) {
            const refreshed = await session.refresh();
            if (refreshed) {
                return await this.#commonMethods({
                    endpoint, method, headers, body, query, showError,
                    isRetry: true,
                });
            }
        }

        if (!response.ok) {
            let error;
            try {
                const result = await response.json();
                // Backend error bodies use `message` (utils::response::Response);
                // keep `error` too for any endpoints that use that key instead.
                error = result.error || result.message || "Response Not Okay";
            } catch (e) {
                console.error(e);
                if (response.status === 400) {
                    error = "Bad request, Check the request body";
                } else if (response.status === 404) {
                    error = "Not found, Check the api route";
                } else {
                    error = "Response Not Okay";
                }
            }

            if (showError) {
                toast.setNotification({
                    type: 'error',
                    message: error,
                })
            }

            return new FetchResult({
                ok: false,
                status: response.status,
                data: null,
                error: error,
            })
        }

        let result;
        try {
            result = await response.json();
        } catch (e) {
            console.error(e);

            // Not `error` — that was scoped to the !response.ok branch above,
            // and naming it here threw a ReferenceError that buried whatever
            // actually went wrong.
            const parseError = 'Error Parsing Response';

            if (showError) {
                toast.setNotification({
                    type: 'error',
                    message: parseError,
                })
            }

            return new FetchResult({
                ok: false,
                status: response.status,
                data: null,
                error: parseError,
            })
        }

        return new FetchResult({
            ok: true,
            status: response.status,
            data: result,
            error: null,
        })
    }
}

class FetchResult {
    constructor({
        ok,
        status,
        data,
        error
    }) {
        this.ok = ok;
        this.status = status;
        this.data = data;
        this.error = error;
    }
}

window.addEventListener("load", () => {
    window.Fetcher = Fetcher;
});

// Also expose as ES-module exports so `type="module"` scripts (e.g. auth.js)
// can import it directly without relying on the window global / load order.
export { Fetcher, FetchResult };