use crate::domain::{SubscriberEmail, SubscriberName};
use crate::email_client::sendgrid::{Content, MailSendRequest, Personalization, Subscriber};
use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

pub struct EmailClient {
    http_client: Client,
    base_url: reqwest::Url,
    sender: SubscriberEmail,
    authorization_token: Secret<String>,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        timeout: std::time::Duration,
    ) -> Self {
        let http_client = Client::builder().timeout(timeout).build().unwrap();
        let base_url = base_url.parse().expect("Invalid base url");
        let authorization_token =
            Secret::new("Bearer ".to_string() + authorization_token.expose_secret());

        Self {
            http_client,
            base_url,
            sender,
            authorization_token,
        }
    }
    pub async fn send_email(
        &self,
        email: &SubscriberEmail,
        name: &SubscriberName,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = self.base_url.join("/v3/mail/send").unwrap();
        let request_body = MailSendRequest {
            personalizations: vec![Personalization {
                to: vec![Subscriber {
                    email: email.as_ref(),
                    name: name.as_ref(),
                }],
                subject,
            }],
            content: vec![
                Content {
                    r#type: "text/plain",
                    value: text_content,
                },
                Content {
                    r#type: "text/html",
                    value: html_content,
                },
            ],
            from: Subscriber {
                email: self.sender.as_ref(),
                name: "zero2prod",
            },
            reply_to: Subscriber {
                email: self.sender.as_ref(),
                name: "zero2prod",
            },
        };
        self.http_client
            .post(url)
            .header("Authorization", self.authorization_token.expose_secret())
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

mod sendgrid {
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct MailSendRequest<'a> {
        pub personalizations: Vec<Personalization<'a>>,
        pub content: Vec<Content<'a>>,
        pub from: Subscriber<'a>,
        pub reply_to: Subscriber<'a>,
    }

    #[derive(Serialize)]
    pub struct Personalization<'a> {
        pub to: Vec<Subscriber<'a>>,
        pub subject: &'a str,
    }

    #[derive(Serialize)]
    pub struct Subscriber<'a> {
        pub email: &'a str,
        pub name: &'a str,
    }

    #[derive(Serialize)]
    pub struct Content<'a> {
        pub r#type: &'a str,
        pub value: &'a str,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{SubscriberEmail, SubscriberName};
    use crate::email_client::EmailClient;
    use claims::{assert_err, assert_ok};
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::faker::name::en::Name;
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::{any, header, header_regex, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    struct MailSendRequestBodyMatcher;

    impl wiremock::Match for MailSendRequestBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("personalizations").is_some()
                    && body.get("content").is_some()
                    && body.get("from").is_some()
                    && body.get("reply_to").is_some()
            } else {
                false
            }
        }
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(header_regex("Authorization", "Bearer .*"))
            .and(header("Content-Type", "application/json"))
            .and(path("/v3/mail/send"))
            .and(method("POST"))
            .and(MailSendRequestBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = email_client
            .send_email(&email(), &name(), &subject(), &content(), &content())
            .await;
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &name(), &subject(), &content(), &content())
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &name(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        let response = ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(180));
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &name(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn name() -> SubscriberName {
        SubscriberName::parse(Name().fake()).unwrap()
    }

    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(
            base_url,
            email(),
            Secret::new(Faker.fake()),
            std::time::Duration::from_millis(50),
        )
    }
}
