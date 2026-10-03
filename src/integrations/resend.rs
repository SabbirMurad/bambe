use resend_rs::types::CreateEmailBaseOptions;
use resend_rs::{Resend, Result};
use std::env;

const BRAND: &str = "Bambe";
const ACCENT: &str = "#FAAA39"; // light orange — matches --primary in common.css
const ACCENT_DARK: &str = "#E8951F";
const ON_ACCENT: &str = "#2A1B06"; // dark ink on an orange fill; white is 1.9:1 here
const PAGE_BG: &str = "#FCFBF9";
const PANEL_BG: &str = "#F6F5F2";
const HAIRLINE: &str = "#E8E5E0";
const INK: &str = "#1A1A18";
const MUTED: &str = "#6A665F";
const CODE_EXPIRE_MIN: &str = "15"; // must match CODE_EXPIRE_TIME in the auth handlers

/// Absolute base for links in email. No domain is configured anywhere else in
/// the project, so it comes from the environment and falls back to the dev
/// server rather than shipping a guess.
fn site_url() -> String {
    env::var("SITE_URL").unwrap_or_else(|_| "http://localhost:8081".to_string())
}

/// Wrap page-specific `inner` rows in the shared, self-contained (no external
/// images) email shell: brand header band + footer. Bulletproof table layout
/// with inline styles so it renders in Gmail, Apple Mail, and Outlook.
fn shell(title: &str, preheader: &str, inner: &str) -> String {
    let support_url = format!("{}/account/support", site_url());

    format!(
        r#"<!DOCTYPE html>
<html lang="en" xmlns="http://www.w3.org/1999/xhtml">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="X-UA-Compatible" content="IE=edge">
  <title>{title} · {BRAND}</title>
  <style>
    .preheader {{ display:none !important; visibility:hidden; opacity:0; color:transparent; height:0; width:0; overflow:hidden; mso-hide:all; }}
    @media only screen and (max-width:600px) {{
      .card {{ width:100% !important; border-radius:0 !important; }}
      .px {{ padding-left:24px !important; padding-right:24px !important; }}
      .code {{ font-size:34px !important; letter-spacing:8px !important; }}
    }}
  </style>
</head>
<body style="margin:0; padding:0; background:{PAGE_BG}; -webkit-text-size-adjust:100%; -ms-text-size-adjust:100%;">
  <span class="preheader">{preheader}</span>
  <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background:{PAGE_BG};">
    <tr>
      <td align="center" style="padding:32px 12px;">
        <table role="presentation" class="card" width="600" cellpadding="0" cellspacing="0" border="0" style="width:600px; max-width:600px; background:#ffffff; border-radius:16px; overflow:hidden; box-shadow:0 1px 4px rgba(26,26,24,0.08);">
          <tr>
            <td align="center" style="background:{ACCENT}; padding:26px 24px;">
              <span style="font-family:Arial,Helvetica,sans-serif; font-size:26px; font-weight:800; letter-spacing:-0.5px; color:{ON_ACCENT};">{BRAND}</span>
            </td>
          </tr>
          {inner}
          <tr>
            <td align="center" style="background:{PAGE_BG}; padding:24px 48px; font-family:Arial,Helvetica,sans-serif; border-top:1px solid {HAIRLINE};">
              <p style="margin:0 0 6px 0; font-size:13px; color:{MUTED};">Need help? <a href="{support_url}" style="color:{ACCENT_DARK}; text-decoration:none; font-weight:600;">Contact support</a></p>
              <p style="margin:0; font-size:12px; color:#94908A;">© 2026 {BRAND}. All rights reserved.</p>
            </td>
          </tr>
        </table>
        <table role="presentation" width="600" cellpadding="0" cellspacing="0" border="0" style="width:600px; max-width:600px;">
          <tr>
            <td align="center" style="padding:20px 24px 0; font-family:Arial,Helvetica,sans-serif;">
              <p style="margin:0; font-size:11px; color:#C4C0B9;">This is an automated message from {BRAND}. Please don't reply to this email.</p>
            </td>
          </tr>
        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#
    )
}

/// A one-time-code email (sign-up / sign-in / password reset).
fn code_email(heading: &str, intro: &str, code: &str, note: &str) -> String {
    let inner = format!(
        r#"<tr>
            <td class="px" style="padding:40px 48px 4px 48px; font-family:Arial,Helvetica,sans-serif;">
              <h1 style="margin:0 0 12px 0; font-size:24px; line-height:1.3; color:{INK}; font-weight:800;">{heading}</h1>
              <p style="margin:0; font-size:16px; line-height:1.6; color:{MUTED};">{intro}</p>
            </td>
          </tr>
          <tr>
            <td class="px" style="padding:28px 48px 4px 48px;">
              <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
                <tr>
                  <td align="center" style="background:{PANEL_BG}; border:1px solid {HAIRLINE}; border-radius:12px; padding:22px 16px;">
                    <div class="code" style="font-family:'Courier New',Courier,monospace; font-size:40px; font-weight:700; letter-spacing:12px; color:{INK}; padding-left:12px;">{code}</div>
                  </td>
                </tr>
              </table>
            </td>
          </tr>
          <tr>
            <td class="px" align="center" style="padding:16px 48px 0 48px; font-family:Arial,Helvetica,sans-serif;">
              <p style="margin:0; font-size:14px; color:{MUTED};">This code expires in <strong style="color:{INK};">{CODE_EXPIRE_MIN} minutes</strong>. Please don't share it with anyone.</p>
            </td>
          </tr>
          <tr>
            <td class="px" style="padding:32px 48px 40px 48px; font-family:Arial,Helvetica,sans-serif;">
              <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
                <tr><td style="border-top:1px solid {HAIRLINE}; padding-top:24px;">
                  <p style="margin:0; font-size:13px; line-height:1.6; color:{MUTED};">{note}</p>
                </td></tr>
              </table>
            </td>
          </tr>"#
    );
    shell(
        heading,
        &format!("Your {BRAND} code is {code} — expires in {CODE_EXPIRE_MIN} minutes."),
        &inner,
    )
}

/// Sign-up email-verification code (new account).
pub fn sign_up_verification_code_template(code: &str) -> String {
    code_email(
        "Verify your email",
        "Enter the code below to confirm your email and finish setting up your Bambe account.",
        code,
        "Didn't sign up for Bambe? You can safely ignore this email — no account is created without this code.",
    )
}

/// Sign-in verification code (2-step login).
pub fn sign_in_verification_code_template(code: &str) -> String {
    code_email(
        "Confirm it's you",
        "Use the code below to finish signing in to your Bambe account.",
        code,
        "Didn't try to sign in? Someone may have your password — we recommend changing it right away.",
    )
}

/// Password-reset verification code.
pub fn password_reset_verification_code_template(code: &str) -> String {
    code_email(
        "Reset your password",
        "Use the code below to reset the password on your Bambe account.",
        code,
        "Didn't request a password reset? You can safely ignore this email — your password won't change.",
    )
}

/// Format minor units for display in an email. The rest of the project keeps
/// money as `Cents` and formats only at the edge; this is one such edge.
fn money(cents: i64, currency: &str) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    let symbol = match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "BDT" => "৳",
        _ => "",
    };
    if symbol.is_empty() {
        format!("{}{} {}.{:02}", sign, currency, abs / 100, abs % 100)
    } else {
        format!("{}{}{}.{:02}", sign, symbol, abs / 100, abs % 100)
    }
}

/// An order-lifecycle email: heading, a sentence, a labelled detail block, and
/// an optional button back into the account area.
fn order_email(
    heading: &str,
    intro: &str,
    rows: &[(&str, String)],
    cta: Option<(&str, String)>,
    note: &str,
) -> String {
    let detail_rows: String = rows
        .iter()
        .map(|(label, value)| {
            format!(
                r#"<tr>
                    <td style="padding:10px 0; font-size:14px; color:{MUTED}; border-bottom:1px solid {HAIRLINE};">{label}</td>
                    <td align="right" style="padding:10px 0; font-size:14px; font-weight:700; color:{INK}; border-bottom:1px solid {HAIRLINE};">{value}</td>
                  </tr>"#
            )
        })
        .collect();

    let cta_row = match cta {
        Some((label, url)) => format!(
            r#"<tr>
            <td class="px" align="center" style="padding:28px 48px 0 48px; font-family:Arial,Helvetica,sans-serif;">
              <table role="presentation" cellpadding="0" cellspacing="0" border="0">
                <tr><td align="center" style="background:{ACCENT}; border-radius:10px;">
                  <a href="{url}" style="display:inline-block; padding:14px 30px; font-size:15px; font-weight:700; color:{ON_ACCENT}; text-decoration:none;">{label}</a>
                </td></tr>
              </table>
            </td>
          </tr>"#
        ),
        None => String::new(),
    };

    let inner = format!(
        r#"<tr>
            <td class="px" style="padding:40px 48px 4px 48px; font-family:Arial,Helvetica,sans-serif;">
              <h1 style="margin:0 0 12px 0; font-size:24px; line-height:1.3; color:{INK}; font-weight:800;">{heading}</h1>
              <p style="margin:0; font-size:16px; line-height:1.6; color:{MUTED};">{intro}</p>
            </td>
          </tr>
          <tr>
            <td class="px" style="padding:24px 48px 0 48px; font-family:Arial,Helvetica,sans-serif;">
              <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" style="background:{PANEL_BG}; border:1px solid {HAIRLINE}; border-radius:12px; padding:6px 20px;">
                {detail_rows}
              </table>
            </td>
          </tr>
          {cta_row}
          <tr>
            <td class="px" style="padding:32px 48px 40px 48px; font-family:Arial,Helvetica,sans-serif;">
              <table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
                <tr><td style="border-top:1px solid {HAIRLINE}; padding-top:24px;">
                  <p style="margin:0; font-size:13px; line-height:1.6; color:{MUTED};">{note}</p>
                </td></tr>
              </table>
            </td>
          </tr>"#
    );

    shell(heading, intro, &inner)
}

fn orders_url() -> String {
    format!("{}/account/orders", site_url())
}

/// Order received. Sent the moment checkout succeeds.
pub fn order_placed_template(
    number: &str,
    item_count: u32,
    total: i64,
    currency: &str,
    payment_summary: &str,
    delivery: &str,
) -> String {
    order_email(
        "Thanks for your order",
        "We've got it. You'll hear from us again as soon as it's on its way.",
        &[
            ("Order", number.to_string()),
            ("Items", item_count.to_string()),
            ("Delivery", delivery.to_string()),
            ("Payment", payment_summary.to_string()),
            ("Total", money(total, currency)),
        ],
        Some(("View your order", orders_url())),
        "Need to change something? Reply to this order from your account within the hour and we'll do our best.",
    )
}

/// Dispatched. `tracking` is shown only when there is one.
pub fn order_shipped_template(number: &str, tracking: Option<&str>, delivery: &str) -> String {
    let mut rows = vec![
        ("Order", number.to_string()),
        ("Delivery", delivery.to_string()),
    ];
    if let Some(tracking) = tracking {
        rows.push(("Tracking", tracking.to_string()));
    }

    order_email(
        "Your order is on its way",
        "It's left us and is with the courier now.",
        &rows,
        Some(("Track your order", format!("{}/account/tracking", site_url()))),
        "Delivery estimates are the courier's, not ours — most parcels arrive inside the window, a few take a day longer.",
    )
}

/// Delivered, per the courier or staff marking it so.
pub fn order_delivered_template(number: &str) -> String {
    order_email(
        "Your order has arrived",
        "The courier has marked it delivered. We hope it's everything you wanted.",
        &[("Order", number.to_string())],
        Some(("Leave a review", orders_url())),
        "Something not right? You can start a return from your account for 30 days after delivery.",
    )
}

/// Cancelled, by the customer or by staff.
pub fn order_cancelled_template(number: &str, refund: Option<(i64, &str)>) -> String {
    let mut rows = vec![("Order", number.to_string())];
    if let Some((amount, currency)) = refund {
        rows.push(("Refunded", money(amount, currency)));
    }

    order_email(
        "Your order was cancelled",
        match refund {
            Some(_) => "It won't be shipped, and the payment has been returned.",
            None => "It won't be shipped. Nothing was charged.",
        },
        &rows,
        Some(("Browse the shop", site_url())),
        "Card refunds usually land in 3–5 working days, depending on your bank.",
    )
}

/// Money returned — from a manual refund or a resolved return.
pub fn refund_issued_template(number: &str, amount: i64, currency: &str) -> String {
    order_email(
        "Your refund is on its way",
        "We've issued the refund against your original payment method.",
        &[
            ("Order", number.to_string()),
            ("Refunded", money(amount, currency)),
        ],
        Some(("View your order", orders_url())),
        "Card refunds usually land in 3–5 working days, depending on your bank.",
    )
}

/// A return moved on — approved, rejected, or received.
pub fn return_update_template(number: &str, status: &str, staff_note: Option<&str>) -> String {
    let mut rows = vec![
        ("Order", number.to_string()),
        ("Return status", status.to_string()),
    ];
    if let Some(note) = staff_note {
        if !note.trim().is_empty() {
            rows.push(("Note from us", note.to_string()));
        }
    }

    order_email(
        "An update on your return",
        "Your return request has moved on.",
        &rows,
        Some(("View your returns", format!("{}/account/returns", site_url()))),
        "Questions about a return? Reply through support from your account and we'll pick it up.",
    )
}

pub async fn send_email(email: &str, sub: &str, body: &str) -> Result<(), resend_rs::Error> {
    let resend_api = env::var("RESEND_API_KEY").expect("RESEND_API_KEY must be set on .env file");

    let resend = Resend::new(&resend_api);

    // The sending domain has to be one verified in Resend; override it with
    // MAIL_FROM once Bambe has its own.
    let from = env::var("MAIL_FROM")
        .unwrap_or_else(|_| "Bambe <no-reply@sabbirhassan.com>".to_string());
    let to = [email];

    let email = CreateEmailBaseOptions::new(&from, to, sub).with_html(body);

    let result = resend.emails.send(email).await;

    if let Err(err) = result {
        log::error!("{:?}", err);
        return Err(err);
    }

    Ok(())
}
