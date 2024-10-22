use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

use crate::domain::{NewSubscriber, SubscriberEmail};
use crate::email_client::sendgrid::{Content, MailSendRequest, Personalization, Subscriber};

pub struct EmailClient {
    http_client: Client,
    // TODO use reqwest::Url instead of String
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: Secret<String>,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
    ) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            sender,
            authorization_token,
        }
    }
    pub async fn send_email(
        &self,
        new_subscriber: NewSubscriber,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/v3/mail/send", self.base_url);
        let request_body = MailSendRequest {
            personalizations: vec![Personalization {
                to: vec![Subscriber {
                    email: new_subscriber.email.as_ref().to_owned(),
                    name: new_subscriber.name.as_ref().to_owned(),
                }],
                subject: subject.to_string(),
            }],
            content: vec![
                Content {
                    r#type: "text/plain".to_string(),
                    value: text_content.to_string(),
                },
                Content {
                    r#type: "text/html".to_string(),
                    value: html_content.to_string(),
                },
            ],
            from: Subscriber {
                email: self.sender.as_ref().to_owned(),
                name: "zero2prod".to_string(),
            },
            reply_to: Subscriber {
                email: self.sender.as_ref().to_owned(),
                name: "zero2prod".to_string(),
            },
        };
        self.http_client
            .post(&url)
            .header(
                "Authorization",
                "Bearer ".to_string() + self.authorization_token.expose_secret(),
            )
            .json(&request_body)
            .send()
            .await?;
        Ok(())
    }
}

mod sendgrid {
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct MailSendRequest {
        pub personalizations: Vec<Personalization>,
        pub content: Vec<Content>,
        pub from: Subscriber,
        pub reply_to: Subscriber,
    }

    #[derive(Serialize)]
    pub struct Personalization {
        pub to: Vec<Subscriber>,
        pub subject: String,
    }

    #[derive(Serialize)]
    pub struct Subscriber {
        pub email: String,
        pub name: String,
    }

    #[derive(Serialize)]
    pub struct Content {
        pub r#type: String,
        pub value: String,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::faker::name::en::Name;
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::{header, header_regex, method, path};
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
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), sender, Secret::new(Faker.fake()));

        Mock::given(header_regex("Authorization", "Bearer .*"))
            .and(header("Content-Type", "application/json"))
            .and(path("v3/mail/send"))
            .and(method("POST"))
            .and(MailSendRequestBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let name = SubscriberName::parse(Name().fake()).unwrap();
        let new_subscriber = NewSubscriber { email, name };
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        let _ = email_client
            .send_email(new_subscriber, &subject, &content, &content)
            .await;
    }
}
