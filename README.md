lila-push
=========

Web push microservice for [lichess.org](https://lichess.org).

Setup
-----

1. Generate a private key.

   ```
   openssl ecparam -name prime256v1 -genkey -noout -out private.pem
   ```

2. Print public key and set `push.web.vapid_public_key` in lila configuration:

   ```
   openssl ec -in private.pem -pubout -outform DER | tail -c 65 | base64 | tr -d '\n' && echo
   ```

3. Run microservice:

   ```
   cargo run --release -- --vapid private.pem --vapid-subject mailto:contact@lichess.org
   ```

Usage
-----

```
cargo run -- --help
```

HTTP API
--------

### `POST /` send a push message

```javascript
{
  "subs": [{
    "endpoint": "https://fcm.googleapis.com/fcm/send/dpH5lCsTSSM:APA91bHqjZxM0VImWWqDRN7U0a3AycjUf4O-byuxb_wJsKRaKvV_iKw56s16ekq6FUqoCF7k2nICUpd8fHPxVTgqLunFeVeB9lLCQZyohyAztTH8ZQL9WCxKpA6dvTG_TUIhQUFq_n",
    "keys": {
      "p256dh": "BLQELIDm-6b9Bl07YrEuXJ4BL_YBVQ0dvt9NQGGJxIQidJWHPNa9YrouvcQ9d7_MqzvGS9Alz60SZNCG3qfpk=",
      "auth": "4vQK-SvRAN5eo-8ASlrwA=="
    }
  }],
  "payload": "lorem ipsum", // could be json encoded as a string
  "ttl": 43200 // 12 hour limit to deliver
}
```

```json
{
  "https://fcm.googleapis.com/fcm/send/dpH5lCsTSSM:AP...": "ok"
}
```

response | description
--- | ---
ok | **success**
endpoint_not_valid | Subscription is invalid and will never be valid
not_implemented | Endpoint does not support encryption algorithm
... | Other errors

License
-------

lila-push is licensed under the GNU Affero General Public License 3.0 (or any
later version at your option). See COPYING for the full license text.
