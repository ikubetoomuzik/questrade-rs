# Questrade-rs

A [Questrade API client](https://www.questrade.com/api/home) implemented in pure async rust.

## Examples

```rust
let api = Questrade::new();
api.authenticate(&refresh_token, false).await?;

let account = api.accounts().await?.pop().expect("No accounts registered for this user");
api.account_balance(&account.number).await?
```

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>