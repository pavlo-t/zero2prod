use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use sqlx::types::{chrono, uuid};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(
    form: web::Form<FormData>,
    connection: web::Data<PgConnection>,
) -> impl Responder {
    sqlx::query!(
        // language=SQL
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(connection.get_ref())
    .await;
    HttpResponse::Ok().finish()
}
