# **About Ppaass**

Ppaass project have a proxy and a agent, and build based on rust.

## **Ppaass proxy**

The proxy part of ppaass running in cloud side, it will proxy the stream from agent side and access the target of internet, the protocol between proxy and agent is encrypted.

## **Ppaass agent**

The agent part of ppaass running in the client side, it will work as a socks5/http agent, relay the TCP/UDP stream to ppaass proxy then to the target in the internet.
