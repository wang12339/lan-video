use std::time::Duration;

use crate::config::AppConfig;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

const MAX_RECIPIENT_LEN: usize = 254;
const SMTP_SEND_TIMEOUT: Duration = Duration::from_secs(20);
const SMTP_MAX_ATTEMPTS: usize = 2;
const SMTP_RETRY_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct EmailService {
    config: AppConfig,
}

impl EmailService {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }

    fn mask_email(email: &str) -> String {
        let Some(at_pos) = email.find('@') else {
            return "***".to_string();
        };
        let (local, domain) = email.split_at(at_pos);
        let domain = &domain[1..];
        let masked_local = Self::mask_part(local, 1, 1, 2);
        let masked_domain = Self::mask_part(domain, 1, 3, 4);
        format!("{}@{}", masked_local, masked_domain)
    }

    fn mask_part(part: &str, keep_head: usize, keep_tail: usize, min_len: usize) -> String {
        let chars: Vec<char> = part.chars().collect();
        if chars.len() <= min_len {
            return "***".to_string();
        }
        let head: String = chars[..keep_head].iter().collect();
        let tail: String = chars[chars.len() - keep_tail..].iter().collect();
        format!("{}***{}", head, tail)
    }

    fn is_safe_recipient(to: &str) -> bool {
        !to.is_empty()
            && to.len() <= MAX_RECIPIENT_LEN
            && !to.chars().any(|c| c.is_control() || c.is_whitespace())
            && !to.contains(['"', '<', '>'])
    }

    pub fn is_configured(&self) -> bool {
        !self.config.smtp_host.is_empty()
            && !self.config.smtp_username.is_empty()
            && !self.config.smtp_password.is_empty()
    }

    pub async fn send_password_reset(&self, email: &str, username: &str, reset_url: &str) {
        let subject = "重置您的 Atmos 密码";
        let safe_username = Self::html_escape(username);
        let safe_url = Self::html_escape(reset_url);
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; color: #333;">
  <h2 style="color: #ff4433;">重置密码</h2>
  <p>您好 <strong>{}</strong>,</p>
  <p>请点击以下按钮重置您的密码：</p>
  <p style="text-align: center; margin: 30px 0;">
    <a href="{}" style="background: #ff4433; color: white; padding: 12px 32px; text-decoration: none; border-radius: 8px; font-weight: 500;">重置密码</a>
  </p>
  <p style="color: #666; font-size: 13px;">此链接将在 <strong>1 小时</strong>后过期。</p>
  <p style="color: #666; font-size: 13px;">如果按钮无法点击，请复制以下链接到浏览器打开：</p>
  <p style="color: #666; font-size: 13px; word-break: break-all;">{}</p>
  <hr style="border: none; border-top: 1px solid #eee; margin: 24px 0;">
  <p style="color: #999; font-size: 12px;">如果不是您本人操作，请忽略此邮件。</p>
  <p style="color: #999; font-size: 12px;">Atmos Video 团队</p>
</body>
</html>"#,
            safe_username, safe_url, safe_url
        );
        self.send(email, subject, &body).await;
    }

    pub async fn send_email_verification(&self, email: &str, username: &str, verify_url: &str) {
        let subject = "验证您的 Atmos 邮箱";
        let safe_username = Self::html_escape(username);
        let safe_url = Self::html_escape(verify_url);
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: -apple-system, BlinkMacSystemFont, sans-serif; padding: 20px; color: #333;">
  <h2 style="color: #ff4433;">邮箱验证</h2>
  <p>您好 <strong>{}</strong>,</p>
  <p>请点击以下按钮验证您的邮箱：</p>
  <p style="text-align: center; margin: 30px 0;">
    <a href="{}" style="background: #ff4433; color: white; padding: 12px 32px; text-decoration: none; border-radius: 8px; font-weight: 500;">验证邮箱</a>
  </p>
  <p style="color: #666; font-size: 13px;">此链接将在 <strong>24 小时</strong>后过期。</p>
  <p style="color: #666; font-size: 13px;">如果按钮无法点击，请复制以下链接到浏览器打开：</p>
  <p style="color: #666; font-size: 13px; word-break: break-all;">{}</p>
  <hr style="border: none; border-top: 1px solid #eee; margin: 24px 0;">
  <p style="color: #999; font-size: 12px;">如果不是您本人操作，请忽略此邮件。</p>
  <p style="color: #999; font-size: 12px;">Atmos Video 团队</p>
</body>
</html>"#,
            safe_username, safe_url, safe_url
        );
        self.send(email, subject, &body).await;
    }

    fn build_mailer(&self) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
        let creds = Credentials::new(
            self.config.smtp_username.clone(),
            self.config.smtp_password.clone(),
        );

        // Use different encryption based on port
        // Port 465: implicit SSL/TLS
        // Port 587: STARTTLS
        let builder = if self.config.smtp_port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.smtp_host)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
        };

        builder
            .map(|b| b.port(self.config.smtp_port).credentials(creds).build())
            .map_err(|e| e.to_string())
    }

    fn build_message(
        from: lettre::message::Mailbox,
        to: lettre::message::Mailbox,
        subject: &str,
        body: &str,
    ) -> Result<Message, lettre::error::Error> {
        Message::builder()
            .from(from)
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body.to_string())
    }

    async fn send(&self, to: &str, subject: &str, body: &str) {
        let masked_to = Self::mask_email(to);

        if !self.is_configured() {
            tracing::info!(
                "SMTP not configured — would send email to {}: subject={}",
                masked_to,
                subject
            );
            return;
        }

        if !Self::is_safe_recipient(to) {
            tracing::error!(
                recipient = %masked_to,
                "Refusing to send email: unsafe recipient address"
            );
            return;
        }

        let to_addr: lettre::message::Mailbox = match to.parse() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::error!("Invalid email address '{}': {}", masked_to, e);
                return;
            }
        };

        let from: lettre::message::Mailbox = self
            .config
            .smtp_from
            .parse()
            .unwrap_or_else(|_| "noreply@localhost".parse().unwrap());

        tracing::info!(
            "Sending email to {}: subject={} (SMTP: {}:{})",
            masked_to,
            subject,
            self.config.smtp_host,
            self.config.smtp_port
        );

        let mailer = match self.build_mailer() {
            Ok(mailer) => mailer,
            Err(e) => {
                tracing::error!("Failed to create SMTP transport: {}", e);
                return;
            }
        };

        let mut last_error: Option<String> = None;
        for attempt in 1..=SMTP_MAX_ATTEMPTS {
            let email = match Self::build_message(from.clone(), to_addr.clone(), subject, body) {
                Ok(email) => email,
                Err(e) => {
                    tracing::error!("Failed to build email: {}", e);
                    return;
                }
            };

            match tokio::time::timeout(SMTP_SEND_TIMEOUT, mailer.send(email)).await {
                Ok(Ok(_)) => {
                    tracing::info!("Email sent successfully to {}", masked_to);
                    return;
                }
                Ok(Err(e)) => last_error = Some(e.to_string()),
                Err(_) => last_error = Some("timed out".to_string()),
            }

            if attempt < SMTP_MAX_ATTEMPTS {
                tracing::warn!(
                    attempt,
                    error = %last_error.as_deref().unwrap_or("unknown"),
                    "SMTP send attempt failed, retrying"
                );
                tokio::time::sleep(SMTP_RETRY_DELAY).await;
            }
        }

        tracing::error!(
            "Failed to send email to {} after {} attempts: {}",
            masked_to,
            SMTP_MAX_ATTEMPTS,
            last_error.as_deref().unwrap_or("unknown")
        );
    }
}
