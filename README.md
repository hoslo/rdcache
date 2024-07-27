# rdcache
[![Crates.io][crates-badge]][crates-url]
[![MIT/Apache-2 licensed][license-badge]][license-url]

[crates-badge]: https://img.shields.io/crates/v/rdcache.svg
[crates-url]: https://crates.io/crates/rdcache
[license-badge]: https://img.shields.io/crates/l/rdcache.svg
[license-url]: LICENSE

Rust version of [rockscache](https://github.com/dtm-labs/rockscache)

## Features
- Execute an async task only once for the same key at the same time and diffrent application.
- Use MessagePack to cache data.

## Example
```rust
use std::time::Duration;

use rdcache::{Client, Options};
use rustis::client::Client as RedisClient;

#[tokio::main]
async fn main() {
    let rdb = RedisClient::connect("127.0.0.1:6379").await.unwrap();
    let client = Client::new(rdb, Options::default());

    let key = "key";

    let r = client
        .fetch(key, Duration::from_secs(600), || async {
            println!("Fetching data from the database");
            Ok(Some("data".to_string()))
        })
        .await;

    println!("{:?}", r);

    client.tag_as_deleted(key).await.unwrap();

    let r = client
        .fetch(key, Duration::from_secs(600), || async {
            println!("Fetching data from the database");
            Ok(Some("data2".to_string()))
        })
        .await;

    println!("{:?}", r);
}

```

The output will be like:
```
Fetching data from the database
Ok(Some("data"))
Fetching data from the database
Ok(Some("data2"))
```