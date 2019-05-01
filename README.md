lila-push
=========

Web push microservice for lichess.org.

VAPID setup
-----------

Generate a private key.

```
openssl ecparam -name prime256v1 -genkey -noout -out private.pem
```

Print public key:

```
openssl ec -in private.pem -pubout -outform DER|tail -c 65|base64|tr '/+' '_-'|tr -d '\n'
```

Print private key:

```
openssl ec -in private.pem -outform DER | base64 | tr -d "\n"
```

License
-------

lila-push is licensed under the GNU Affero General Public License 3.0 (or any
later version at your option). See COPYING for the full license text.
