=================
Ciruela Protocols
=================

There are three protocols used by ciruela:

1. HTTP just to show you some status pages
2. A protocol to send file data on top of TCP
3. A gossip protocol on top of UDP

In future we will probably expose some JSON API over HTTP just to allow easier
interoperability with the daemon.


TCP Protocol
============


Handshake
---------

TCP protocol starts with HTTP upgrade:

Handshake::

    GET / HTTP/1.1
    Connection: upgrade
    Upgrade: ciruela.v1

Response::

    HTTP/1.1 101 Switching Protocols


Framing
-------

Framing of the protocol is simple: 8-byte big-endian message length
followed by payload of that size::

   +------------+-----------------+
   | size (64b) |     payload     |
   +------------+-----------------+


Serialization
-------------

Payload is serialized using CBOR_. Every packet contains at least two
consecutive values: packet type (string), and packet contents, usually a
dictionary, but it may be anything defined by packet type.

This serialization format is used for messages in both directions.

Packet types (case-sensitive):

* ``AppendDir`` -- deploy data to a new dir
* ``ReplaceDir`` -- deploy and atomically replace the directory


Signing Uploads
---------------

Signature of the upload consists of the following fields packed as the
CBOR length-prefixed array in this specific order:

1. ``path: string`` -- destination path
2. ``image: bytes`` -- binary hashsum of the image (bottom line of the index
  file but in binary form)
3. ``timestamp: u64`` -- milliseconds since unix epoch when the image was
  signed

Ciruela currently only supports ed25519 algorithm for signatures, but more
alorithms (RSA in particular) can be used in future.


Commands
--------


AppendDir
`````````

Schedule a an adding the new directory. This sends only a signed hash of the
directory index and marks this directory as incoming.

.. note:: If different images have been scheduled for upload by different
   peers in the cluster cluster may end up with different images on different
   nodes

If upload for this path and image already exists at node another signature
is added.

If there is no such index on the peer it asks this peer or any other available
connection for the index data itself and subsequently asks for missing chunks
(some chunks may be reused from different image).

Content of the message is a dictionary (cbor object) with the following keys:

* ``path: string`` -- path to put image to
* ``image: bytes`` -- binary hashsum of the image (bottom line of the index
  file but in binary form)
* ``timestamp: u64`` -- milliseconds since unix epoch when the image was signed
* ``signatures: map<bytes, bytes>`` -- map of signatures provided for this
  upload where key is a public key and value is a signature.


ReplaceDir
``````````

Schedule a replacing the directory with the new image. This sends only a
signed hash of the directory index and marks this directory as incoming.

.. note:: If different images have been scheduled for upload by different
   peers in the cluster the one with latest accross the cluster timestamp
   in the signature will win

If there is no such index on the peer it asks this peer or any other available
connection for the index data itself and subsequently asks for missing chunks
(some chunks may be reused from different image).
Content of the message is a dictionary (cbor object) with the following keys:

* ``path: string`` -- path to put image to
* ``old_image: bytes`` -- (optional) binary hashsum of the old image,
  if the dir has different image hash curently deployed, server returns
  error (this might be used for transactional updates)
* ``image: bytes`` -- binary hashsum of the image (bottom line of the index
  file but in binary form)
* ``timestamp: u64`` -- milliseconds since unix epoch when the image was signed
* ``signatures: map<bytes, bytes>`` -- map of signatures provided for this
  upload where key is a public key and value is a signature.

Note: if no ``old_image`` is specified the destination directory is not
checked. Use ``AppendDir`` to atomically update first image.

.. _cbor: http://cbor.io/
