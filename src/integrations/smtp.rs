use lettre::message::{MultiPart, SinglePart, header};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::env;

const CODE_BG_COL: &str = "#9A79F5";
const PROJECT_NAME: &str = "Hasp";
const CODE_EXPIRE_TIME_IN_MINUTE: &str = "10";
const SUPPORT_URL: &str = "https://example.com/support";

pub fn sign_up_verification_code_template(email: &str, code: &str) -> Message {
    let smtp_email = env::var("SMTP_EMAIL").expect("SMTP_EMAIL must be set on .env file");

    let smtp_project_name =
        env::var("SMTP_PROJECT_NAME").expect("SMTP_PROJECT_NAME must be set on .env file");

    let from = format!("{} <{}>", smtp_project_name, smtp_email);

    Message::builder()
    .from(from.parse().unwrap())
    .to(email.parse().unwrap())
    .subject("Verify your email")
    .multipart(
    MultiPart::alternative() // Email can contain plain text and HTML parts
        .singlepart(
            SinglePart::plain(format!("Your email client does not support HTML. Use the following code: {}", code)),
        )
        .singlepart(
            SinglePart::builder()
            .header(header::ContentType::TEXT_HTML)
            .body(format!(r#"
                <html>
                <head>
                    <style>
                        body {{ font-family: Arial, sans-serif; background-color: #f4f4f4; }}
                        .container {{ width: calc(100% - 40px); padding: 48px 20px; background-color: white; border-radius: 10px; text-align: center; color: #000000;}}
                        .code {{ font-size: 24px; color: #fff; font-weight: bold; background-color: {CODE_BG_COL};  padding: 12px 24px; border-radius: 5px; width: 132px; margin: 24px auto; }}
                        .footer {{ color: #888; margin-top: 20px; }}
                        h1 {{ font-size: 24px; margin-bottom: 20px; }}
                        p {{ font-size: 14px; line-height: 19px;}}
                        .footer p {{ font-size: 12px; }}
                    </style>
                </head>
                <body>
                    <div class="container">
                        <h1>Welcome!</h1>
                        <p>Thank you for signing up to <strong>{PROJECT_NAME}</strong>.</p>
                        <p>To verify your email, use the following verification code. Please don't share this code with anyone</p>
                        <p class="code">{code}</p>
                        <p>This code is valid for the next {CODE_EXPIRE_TIME_IN_MINUTE} minutes. If you did not request this, please ignore this email.</p>
                        <p>Thank you, The {PROJECT_NAME} Team</p>

                        <div class="footer">
                          <p>If you have any questions, feel free to contact our support team.</p>
                          <p>{SUPPORT_URL}</p>
                        </div>
                    </div>
                </body>
                </html>
            "#))
        )
    )
    .unwrap()
}

pub fn sign_in_verification_code_template(email: &str, code: &str) -> Message {
    let smtp_email = env::var("SMTP_EMAIL").expect("SMTP_EMAIL must be set on .env file");

    let smtp_project_name =
        env::var("SMTP_PROJECT_NAME").expect("SMTP_PROJECT_NAME must be set on .env file");

    let from = format!("{} <{}>", smtp_project_name, smtp_email);

    Message::builder()
    .from(from.parse().unwrap())
    .to(email.parse().unwrap())
    .subject("Verify your email")
    .multipart(
    MultiPart::alternative() // Email can contain plain text and HTML parts
        .singlepart(
            SinglePart::plain(format!("Your email client does not support HTML. Use the following code: {}", code)),
        )
        .singlepart(
            SinglePart::builder()
            .header(header::ContentType::TEXT_HTML)
            .body(format!(r#"
                <html>
                <head>
                    <style>
                        body {{ font-family: Arial, sans-serif; background-color: #f4f4f4; }}
                        .container {{ width: calc(100% - 40px); padding: 48px 20px; background-color: white; border-radius: 10px; text-align: center; color: #000000;}}
                        .code {{ font-size: 24px; color: #fff; font-weight: bold; background-color: {CODE_BG_COL};  padding: 12px 24px; border-radius: 5px; width: 132px; margin: 24px auto; }}
                        .footer {{ color: #888; margin-top: 20px; }}
                        h1 {{ font-size: 24px; margin-bottom: 20px; }}
                        p {{ font-size: 14px; line-height: 19px;}}
                        .footer p {{ font-size: 12px; }}
                    </style>
                </head>
                <body>
                    <div class="container">
                        <h1>Verification Code</h1>
                        <p>This is to verify that it's really you who are trying to sign in to your <strong>{PROJECT_NAME}</strong> account.</p>
                        <p>To verify, use the following verification code. Please don't share this code with anyone</p>
                        <p class="code">{code}</p>
                        <p>This code is valid for the next {CODE_EXPIRE_TIME_IN_MINUTE} minutes. If you did not request this, please be update your password or check who has access to your password.</p>
                        <p>Thank you, The {PROJECT_NAME} Team</p>
                        <div class="footer">
                          <p>If you have any questions, feel free to contact our support team.</p>
                          <p>{SUPPORT_URL}</p>
                        </div>
                    </div>
                </body>
                </html>
            "#))
        )
    )
    .unwrap()
}

pub fn password_reset_verification_code_template(email: &str, code: &str) -> Message {
    let smtp_email = env::var("SMTP_EMAIL").expect("SMTP_EMAIL must be set on .env file");

    let smtp_project_name =
        env::var("SMTP_PROJECT_NAME").expect("SMTP_PROJECT_NAME must be set on .env file");

    let from = format!("{} <{}>", smtp_project_name, smtp_email);

    Message::builder()
    .from(from.parse().unwrap())
    .to(email.parse().unwrap())
    .subject("Verify your email")
    .multipart(
    MultiPart::alternative() // Email can contain plain text and HTML parts
        .singlepart(
            SinglePart::plain(format!("Your email client does not support HTML. Use the following code: {}", code)),
        )
        .singlepart(
            SinglePart::builder()
            .header(header::ContentType::TEXT_HTML)
            .body(format!(r#"
                <html>
                <head>
                    <style>
                        body {{ font-family: Arial, sans-serif; background-color: #f4f4f4; }}
                        .container {{ width: calc(100% - 40px); padding: 48px 20px; background-color: white; border-radius: 10px; text-align: center; color: #000000;}}
                        .code {{ font-size: 24px; color: #fff; font-weight: bold; background-color: {CODE_BG_COL};  padding: 12px 24px; border-radius: 5px; width: 132px; margin: 24px auto; }}
                        .footer {{ color: #888; margin-top: 20px; }}
                        h1 {{ font-size: 24px; margin-bottom: 20px; }}
                        p {{ font-size: 14px; line-height: 19px;}}
                        .footer p {{ font-size: 12px; }}
                    </style>
                </head>
                <body>
                    <div class="container">
                        <h1>Verification Code</h1>
                        <p>This is to verify that you are trying to reset the password of your <strong>{PROJECT_NAME}</strong> account.</p>
                        <p>To verify, use the following verification code. Please don't share this code with anyone</p>
                        <p class="code">{code}</p>
                        <p>This code is valid for the next {CODE_EXPIRE_TIME_IN_MINUTE} minutes. If you did not request this, please check who has access to your devices or account.</p>
                        <p>Thank you, The {PROJECT_NAME} Team</p>
                        <div class="footer">
                          <p>If you have any questions, feel free to contact our support team.</p>
                          <p>{SUPPORT_URL}</p>
                        </div>
                    </div>
                </body>
                </html>
            "#))
        )
    )
    .unwrap()
}

pub fn send_email(message: Message) -> Result<(), ()> {
    let smtp_email = env::var("SMTP_EMAIL").expect("SMTP_EMAIL must be set on .env file");

    let smtp_password = env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set on .env file");

    let credential = Credentials::new(smtp_email, smtp_password);

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .port(587)
        .credentials(credential)
        .build();

    match mailer.send(&message) {
        Ok(res) => {
            log::info!("{:?}", res);
            Ok(())
        }
        Err(err) => {
            log::error!("{:?}", err);
            Err(())
        }
    }
}
