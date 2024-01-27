# discoboi
Discover OPC-UA servers

## Discovery

NOTE: certs require an FQDN so if you only have an `$IP`, add a mapping from `$NAME` to `$IP` in `/etc/hosts`.

```shell
# From https://github.com/FreeOpcUa/python-opcua/blob/fdf5f3c6c8655c90b0d36ce6a8db54749e8daeb4/examples/generate_certificate.sh#L40
openssl req -x509 -newkey rsa:2048 -keyout my_private_key.pem -out my_cert.pem -days 355 -nodes -subj '/C=FR/CN='"$NAME" -addext 'subjectAltName = URI:urn:'"$IP"
openssl x509 -outform der -in my_cert.pem -out my_cert.der
mkdir -p pki/own pki/private
rm my_cert.pem
mv -v my_cert.der pki/own/cert.der 
mv -v my_private_key.pem pki/private/private.pem
```

```shell
RUST_LOG=debug,opcua=info OPC_URL=opc.tcp://192.168.your.ip:4840/UADiscovery cargo run --locked --frozen --offline -- 
```
