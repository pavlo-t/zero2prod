use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

use crate::domain::SubscriberEmail;
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
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), String> {
        let url = format!("{}/v3/mail/send", self.base_url);
        let request_body = MailSendRequest {
            personalizations: vec![Personalization {
                to: vec![Subscriber {
                    email: recipient.as_ref().to_owned(),
                    // TODO use SubscriberName
                    name: recipient.as_ref().to_owned(),
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
        let builder = self
            .http_client
            .post(&url)
            .header(
                "Authorization",
                "Bearer ".to_string() + self.authorization_token.expose_secret(),
            )
            .json(&request_body);
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
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::any;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn send_email_fails_a_request_to_base_url() {
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(mock_server.uri(), sender, Secret::new(Faker.fake()));

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
    }
}
