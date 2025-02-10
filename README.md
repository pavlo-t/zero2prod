# Zero 2 Prod

Example app from <https://www.zero2prod.com/index.html>

## Run

```shell
docker-compose up db redis -d
sqlx migrate --run

cargo test
cargo run
```

## Stop

```shell
docker-compose down db redis -d
cargo clean
```
