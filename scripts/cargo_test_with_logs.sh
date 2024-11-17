#!/usr/bin/env bash

export RUST_LOG="sqlx=error,info"

#TEST_LOG=true cargo t subscribe_fails_if_there_is_a_fatal_database_error | bunyan
#TEST_LOG=true cargo test --quiet --release newsletters_are_delivered | grep "VERIFY PASSWORD" | bunyan

## Imitate user enumeration attack:
#TEST_LOG=true cargo test --quiet --release non_existing_user_is_rejected | grep "HTTP REQUEST" | bunyan
#TEST_LOG=true cargo test --quiet --release invalid_password_is_rejected | grep "HTTP REQUEST" | bunyan
