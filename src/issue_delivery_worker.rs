use crate::domain::{SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::{configuration::Settings, startup::get_connection_pool};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::time::Duration;
use tracing::{field::display, Span};
use uuid::Uuid;

// TODO there is no expiry mechanism for our idempotency keys.
//  Try designing one as an exercise, using what we learned on background workers as a reference.
pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);
    let email_client = configuration.email_client.client();
    worker_loop(connection_pool, email_client).await
}

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Err(_) => {
                // TODO Almost all errors returned by try_execute_task are transient in nature,
                //  except for invalid subscriber emails - sleeping is not going to fix those.
                //  Try refining the implementation to distinguish between transient and fatal failures,
                //  empowering worker_loop to react appropriately.
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty
    ),
    err
)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = dequeue_task(pool).await?;
    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    let (transaction, issue_id, email) = task.unwrap();
    Span::current()
        .record("newsletter_issue_id", &display(issue_id))
        .record("subscriber_email", &display(&email));
    match get_subscriber(pool, &email).await {
        Ok(subscriber) => {
            let issue = get_issue(pool, issue_id).await?;
            if let Err(e) = email_client
                .send_email(
                    &subscriber.email,
                    &subscriber.name,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await
            {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Failed to deliver issue to a confirmed subscriber. Skipping.",
                );
            }
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber. Their stored contact details are invalid",
            );
        }
    }
    // TODO As you can see, we do not retry when the delivery attempt fails due to a Postmark error.
    //  This could be changed by enhancing issue_delivery_queue - e.g. adding a n_retries and execute_after
    //  columns to keep track of how many attempts have already taken place and how long we should wait before
    //  trying again. Try implementing it as an exercise!
    delete_task(transaction, issue_id, &email).await?;

    Ok(ExecutionOutcome::TaskCompleted)
}

type PgTransaction = Transaction<'static, Postgres>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let query = sqlx::query!(
        // language=SQL
        r#"
        SELECT newsletter_issue_id, subscriber_email
        FROM issue_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    );

    let r = query.fetch_optional(&mut *transaction).await?;
    if let Some(r) = r {
        Ok(Some((
            transaction,
            r.newsletter_issue_id,
            r.subscriber_email,
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        // language=SQL
        r#"
        DELETE FROM issue_delivery_queue
        WHERE
        newsletter_issue_id = $1 AND
        subscriber_email = $2
        "#,
        issue_id,
        email
    );
    transaction.execute(query).await?;
    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        // language=SQL
        r#"
        SELECT title, text_content, html_content
        FROM newsletter_issues
        WHERE
        newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}

struct Subscriber {
    email: SubscriberEmail,
    name: SubscriberName,
}

#[tracing::instrument(skip_all)]
async fn get_subscriber(pool: &PgPool, email: &str) -> Result<Subscriber, anyhow::Error> {
    let r = sqlx::query!(
        // language=SQL
        r#"
        SELECT email, name
        FROM subscriptions
        WHERE email = $1
        "#,
        email
    )
    .fetch_one(pool)
    .await?;

    let email = match SubscriberEmail::parse(r.email) {
        Ok(email) => email,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };
    let name = match SubscriberName::parse(r.name) {
        Ok(name) => name,
        Err(e) => return Err(anyhow::anyhow!(e)),
    };

    Ok(Subscriber { email, name })
}
