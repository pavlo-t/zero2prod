#!/usr/bin/env bash

export RUST_LOG="sqlx=error,info"
TEST_LOG=true cargo t subscribe_fails_if_there_is_a_fatal_database_error | bunyan
