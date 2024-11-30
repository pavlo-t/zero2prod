use crate::domain::{SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::utils::e500;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content_html: String,
    content_text: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(body, pool, email_client),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    body: web::Form<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &subscriber.name,
                        &body.title,
                        &body.content_html,
                        &body.content_text,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid"
                );
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
    name: SubscriberName,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        // language=SQL
        r#"
        SELECT email, name
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| {
        let email = match SubscriberEmail::parse(r.email) {
            Ok(email) => email,
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        let name = match SubscriberName::parse(r.name) {
            Ok(name) => name,
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        Ok(ConfirmedSubscriber { email, name })
    })
    .collect();

    Ok(confirmed_subscribers)
}
